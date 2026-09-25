import { useState } from "react";
import { pickFolder, scanFolder, buildServiceArgs, type ParseResult } from "./lib/scanFolder";
import { type BatProfile } from "./lib/parseBat";
import "./App.css";

function App() {
  const [folder, setFolder] = useState<string | null>(null);
  const [profiles, setProfiles] = useState<BatProfile[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  // Какой профиль сейчас раскрыт + результат его парсинга
  const [activeProfile, setActiveProfile] = useState<string | null>(null);
  const [parseResult, setParseResult] = useState<ParseResult | null>(null);
  const [parsing, setParsing] = useState(false);

  async function handlePickFolder() {
    setError(null);
    setActiveProfile(null);
    setParseResult(null);
    try {
      const picked = await pickFolder();
      if (!picked) return;

      setFolder(picked);
      setLoading(true);

      const found = await scanFolder(picked);
      setProfiles(found);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      console.error("Ошибка сканирования:", e);
      setError(msg);
      setProfiles([]);
    } finally {
      setLoading(false);
    }
  }

  // Клик по профилю → запуск парсера
  async function handleProfileClick(fileName: string) {
    if (!folder) return;

    // Повторный клик по тому же — свернуть
    if (activeProfile === fileName) {
      setActiveProfile(null);
      setParseResult(null);
      return;
    }

    setActiveProfile(fileName);
    setParseResult(null);
    setParsing(true);
    try {
      const result = await buildServiceArgs(folder, fileName);
      setParseResult(result);
      console.log("Результат парсинга:", result);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      console.error("Ошибка парсинга:", e);
      setParseResult({
        ok: false,
        args_clean: "",
        args_escaped: "",
        log: [],
        error: msg,
      });
    } finally {
      setParsing(false);
    }
  }

  return (
    <div className="container">
      <h1>Zapret GUI</h1>

      <button onClick={handlePickFolder} disabled={loading}>
        {loading ? "Сканирование..." : "Выбрать папку zapret"}
      </button>

      {folder && <p className="folder-path">Папка: {folder}</p>}

      {error && <p className="error">⚠ {error}</p>}

      {profiles.length > 0 && (
        <div className="profiles">
          <h2>Найдено профилей: {profiles.length}</h2>
          {profiles.map((p) => (
            <div key={p.fileName}>
              <div
                className={`profile-card ${activeProfile === p.fileName ? "active" : ""}`}
                onClick={() => handleProfileClick(p.fileName)}
              >
                <h3>{p.fileName}</h3>
                <p className="block-count">Блоков: {p.blockCount}</p>
              </div>

              {/* Панель результата парсинга под активной карточкой */}
              {activeProfile === p.fileName && (
                <div className="parse-panel">
                  {parsing && <p className="parse-status">Парсинг...</p>}

                  {parseResult && !parsing && (
                    <>
                      {parseResult.ok ? (
                        <>
                          <div className="parse-section">
                            <span className="parse-label">✓ Чистые аргументы</span>
                            <pre className="parse-args">{parseResult.args_clean}</pre>
                          </div>
                          <div className="parse-section">
                            <span className="parse-label">Экранированные (для sc create)</span>
                            <pre className="parse-args">{parseResult.args_escaped}</pre>
                          </div>
                        </>
                      ) : (
                        <p className="error">⚠ {parseResult.error}</p>
                      )}

                      <details className="parse-log">
                        <summary>Лог парсинга ({parseResult.log.length})</summary>
                        <ol>
                          {parseResult.log.map((line, i) => (
                            <li key={i}>{line}</li>
                          ))}
                        </ol>
                      </details>
                    </>
                  )}
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export default App;