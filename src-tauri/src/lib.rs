use std::fs;
use std::path::Path;
use serde::Serialize;

// ── Структура результата парсинга (уходит в UI) ────────────────
#[derive(Serialize)]
struct ParseResult {
    ok: bool,
    args_clean: String,      // чистые аргументы (без экранирования)
    args_escaped: String,    // экранированные (как эталон Final args)
    log: Vec<String>,        // пошаговый лог
    error: Option<String>,
}

// ── Значения game filter ───────────────────────────────────────
struct GameFilter {
    all: String,
    tcp: String,
    udp: String,
}

// ── Команда: чтение game filter из utils/game_filter.enabled ────
#[tauri::command]
fn read_game_filter(folder: String) -> Result<Vec<String>, String> {
    let gf = get_game_filter(&folder);
    // Возвращаем в UI как [all, tcp, udp]
    Ok(vec![gf.all, gf.tcp, gf.udp])
}

// Внутренняя функция: читает файл-флаг и возвращает значения.
// Логика точно по service.bat :game_switch_status
fn get_game_filter(folder: &str) -> GameFilter {
    let flag_path = Path::new(folder).join("utils").join("game_filter.enabled");

    // Нет файла → disabled → всё 12
    let mode = match fs::read_to_string(&flag_path) {
        Ok(content) => content.lines().next().unwrap_or("").trim().to_lowercase(),
        Err(_) => {
            return GameFilter {
                all: "12".into(),
                tcp: "12".into(),
                udp: "12".into(),
            };
        }
    };

    match mode.as_str() {
        "all" => GameFilter {
            all: "1024-65535".into(),
            tcp: "1024-65535".into(),
            udp: "1024-65535".into(),
        },
        "tcp" => GameFilter {
            all: "1024-65535".into(),
            tcp: "1024-65535".into(),
            udp: "12".into(),
        },
        "udp" => GameFilter {
            all: "1024-65535".into(),
            tcp: "12".into(),
            udp: "1024-65535".into(),
        },
        // непонятное значение → как udp-ветка в батнике (else)
        _ => GameFilter {
            all: "1024-65535".into(),
            tcp: "12".into(),
            udp: "1024-65535".into(),
        },
    }
}

// ── Команда: сборка аргументов службы из .bat ──────────────────
#[tauri::command]
fn build_service_args(folder: String, file_name: String) -> ParseResult {
    let mut log: Vec<String> = Vec::new();

    // Хелпер: пишем и в терминал (println!), и в лог для UI
    macro_rules! step {
        ($($arg:tt)*) => {{
            let msg = format!($($arg)*);
            println!("[build_service_args] {}", msg);
            log.push(msg);
        }};
    }

    step!("Старт. folder={}, file={}", folder, file_name);

    // 1. Прочитать .bat
    let full_path = Path::new(&folder).join(&file_name);
    let content = match fs::read_to_string(&full_path) {
        Ok(c) => c,
        Err(e) => {
            let err = format!("Не прочитать файл: {}", e);
            step!("ОШИБКА: {}", err);
            return ParseResult {
                ok: false,
                args_clean: String::new(),
                args_escaped: String::new(),
                log,
                error: Some(err),
            };
        }
    };
    step!("Файл прочитан, {} байт", content.len());

    // 2. Пути BIN и LISTS (в батнике: %~dp0bin\ и %~dp0lists\)
    let bin_path = format!("{}\\bin\\", folder.trim_end_matches('\\'));
    let lists_path = format!("{}\\lists\\", folder.trim_end_matches('\\'));
    step!("BIN={}", bin_path);
    step!("LISTS={}", lists_path);

    // 3. Game filter
    let gf = get_game_filter(&folder);
    step!("GameFilter: all={} tcp={} udp={}", gf.all, gf.tcp, gf.udp);

    // 4. Найти строку с winws.exe и собрать всю команду.
    //    В батнике команда многострочная через ^ в конце строк.
    //    Склеиваем строки от winws.exe до конца команды.
    let mut raw_cmd = String::new();
    let mut capturing = false;
    for line in content.lines() {
        let trimmed = line.trim();

        if !capturing {
            // Ищем строку, где начинается запуск winws.exe
            if trimmed.to_lowercase().contains("winws.exe") {
                capturing = true;
                step!("Найден winws.exe в строке: {}", trimmed);
                // Берём часть ПОСЛЕ winws.exe"
                if let Some(pos) = line.to_lowercase().find("winws.exe") {
                    let after = &line[pos + "winws.exe".len()..];
                    // Убрать возможную закрывающую кавычку и ^ в конце
                    let after = after.trim_start_matches('"').trim_end();
                    let after = after.strip_suffix('^').unwrap_or(after);
                    raw_cmd.push_str(after.trim());
                    raw_cmd.push(' ');
                }
                continue;
            }
        } else {
            // Уже захватываем: добавляем строки, убирая ^ в конце
            let piece = trimmed.strip_suffix('^').unwrap_or(trimmed);
            raw_cmd.push_str(piece.trim());
            raw_cmd.push(' ');
        }
    }

    if !capturing {
        let err = "Строка с winws.exe не найдена".to_string();
        step!("ОШИБКА: {}", err);
        return ParseResult {
            ok: false,
            args_clean: String::new(),
            args_escaped: String::new(),
            log,
            error: Some(err),
        };
    }
    step!("Сырая команда собрана, {} символов", raw_cmd.len());

    // 5. Токенизация + трансформация.
    //    Разбиваем по пробелам, но уважаем кавычки "...".
    let tokens = tokenize(&raw_cmd);
    step!("Токенов: {}", tokens.len());

    let mut out_tokens: Vec<String> = Vec::new();
    for tok in tokens {
        // 5a. --флаг=значение → --флаг значение (два токена)
        if tok.starts_with("--") && tok.contains('=') {
            let eq = tok.find('=').unwrap();
            let flag = &tok[..eq];
            let value = &tok[eq + 1..];
            out_tokens.push(flag.to_string());
            let transformed = transform_value(value, &bin_path, &lists_path, &gf);
            out_tokens.push(transformed);
        } else if tok.starts_with("--") {
            // Просто флаг
            out_tokens.push(tok);
        } else {
            // Значение без флага (напр. после разбитого =, или standalone)
            let transformed = transform_value(&tok, &bin_path, &lists_path, &gf);
            out_tokens.push(transformed);
        }
    }

    // 6. Собрать чистую строку
    let args_clean = out_tokens.join(" ");
    step!("Чистые args собраны, {} символов", args_clean.len());

    // 7. Экранированная версия (как эталон Final args)
    let args_escaped = escape_for_sc(&args_clean);
    step!("Экранированные args собраны");

    ParseResult {
        ok: true,
        args_clean,
        args_escaped,
        log,
        error: None,
    }
}

// ── Токенизация с уважением к кавычкам ─────────────────────────
fn tokenize(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;

    for ch in s.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                cur.push(ch);
            }
            c if c.is_whitespace() && !in_quotes => {
                if !cur.is_empty() {
                    tokens.push(cur.clone());
                    cur.clear();
                }
            }
            _ => cur.push(ch),
        }
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }
    tokens
}

// ── Трансформация одного значения ──────────────────────────────
// Подставляет %BIN%, %LISTS%, %GameFilter*% и оборачивает пути в кавычки.
fn transform_value(value: &str, bin: &str, lists: &str, gf: &GameFilter) -> String {
    // Снять внешние кавычки, если есть (потом добавим свои для путей)
    let v = value.trim_matches('"');

    // Game filter переменные
    if v == "%GameFilterTCP%" {
        return gf.tcp.clone();
    }
    if v == "%GameFilterUDP%" {
        return gf.udp.clone();
    }
    if v == "%GameFilter%" {
        return gf.all.clone();
    }

    // %BIN%файл → "путь\bin\файл"
    if let Some(rest) = v.strip_prefix("%BIN%") {
        return format!("\"{}{}\"", bin, rest);
    }
    // %LISTS%файл → "путь\lists\файл"
    if let Some(rest) = v.strip_prefix("%LISTS%") {
        return format!("\"{}{}\"", lists, rest);
    }

    // Составные значения (напр. wf-tcp=80,443,%GameFilterTCP%)
    // Заменяем переменные внутри строки
    if v.contains("%GameFilterTCP%") || v.contains("%GameFilterUDP%") || v.contains("%GameFilter%") {
        let replaced = v
            .replace("%GameFilterTCP%", &gf.tcp)
            .replace("%GameFilterUDP%", &gf.udp)
            .replace("%GameFilter%", &gf.all);
        return replaced;
    }

    // Обычное значение — как есть
    v.to_string()
}

// ── Экранирование для sc create ────────────────────────────────
// Превращает "..." → \"...\" (как в эталоне Final args)
fn escape_for_sc(clean: &str) -> String {
    clean.replace('"', "\\\"")
}

// ── Существующие команды сканирования ──────────────────────────
#[tauri::command]
fn list_bat_files(folder: String) -> Result<Vec<String>, String> {
    let dir = Path::new(&folder);
    let entries = fs::read_dir(dir).map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let lower = name.to_lowercase();
        if lower.starts_with("general") && lower.ends_with(".bat") {
            result.push(name);
        }
    }
    Ok(result)
}

#[tauri::command]
fn read_bat_in_folder(folder: String, file_name: String) -> Result<String, String> {
    let full_path = Path::new(&folder).join(&file_name);
    fs::read_to_string(&full_path).map_err(|e| e.to_string())
}

// ── Точка входа ────────────────────────────────────────────────
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            list_bat_files,
            read_bat_in_folder,
            read_game_filter,
            build_service_args
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}