// ── Структура распарсенного профиля ────────────────────────────
export interface BatProfile {
  fileName: string;       // имя файла
  blockCount: number;     // число разделителей --new
  blockNames: string[];   // названия из --comment (для детального вида позже)
}

// ── Парсер: текст батника → структура ──────────────────────────
export function parseBat(fileName: string, content: string): BatProfile {
  // Число блоков = число разделителей --new.
  // \b — граница слова, чтобы не поймать --newfoo и т.п.
  const newMatches = content.match(/--new\b/g);
  const blockCount = newMatches ? newMatches.length : 0;

  // Названия блоков из --comment "..." (сохраняем, но на карточке не показываем).
  const commentRegex = /--comment\s+"([^"]+)"/g;
  const blockNames: string[] = [];
  let match: RegExpExecArray | null;
  while ((match = commentRegex.exec(content)) !== null) {
    blockNames.push(match[1]);
  }

  return { fileName, blockCount, blockNames };
}