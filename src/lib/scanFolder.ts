import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { parseBat, type BatProfile } from "./parseBat";

// Результат сканирования: путь к папке + список профилей.
export interface ScanResult {
  folder: string;
  profiles: BatProfile[];
}

// ── Открыть диалог выбора папки ────────────────────────────────
// Возвращает путь к папке или null, если пользователь отменил.
export async function pickFolder(): Promise<string | null> {
  const selected = await open({
    directory: true,          // выбираем папку, а не файл
    multiple: false,          // только одну
    title: "Выберите папку zapret",
  });
  // open возвращает string | string[] | null.
  // directory:false + multiple:false → string | null.
  return typeof selected === "string" ? selected : null;
}

// ── Просканировать папку: файлы → чтение → парсинг ─────────────
export async function scanFolder(folder: string): Promise<BatProfile[]> {
  // 1. Rust возвращает имена general*.bat файлов.
  const fileNames = await invoke<string[]>("list_bat_files", { folder });

  if (fileNames.length === 0) {
    throw new Error("В выбранной папке нет файлов general*.bat");
  }

  // 2. Для каждого файла: читаем (Rust склеивает путь) и парсим (TS).
  const profiles: BatProfile[] = [];
  for (const fileName of fileNames) {
    const content = await invoke<string>("read_bat_in_folder", {
      folder,
      fileName,
    });
    profiles.push(parseBat(fileName, content));
  }

  return profiles;
}

// ── Результат парсинга аргументов (из Rust) ────────────────────
export interface ParseResult {
  ok: boolean;
  args_clean: string;
  args_escaped: string;
  log: string[];
  error: string | null;
}

// ── Собрать аргументы службы для профиля ───────────────────────
export async function buildServiceArgs(
  folder: string,
  fileName: string
): Promise<ParseResult> {
  return await invoke<ParseResult>("build_service_args", {
    folder,
    fileName,
  });
}