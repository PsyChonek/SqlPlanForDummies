export interface PlanGraphBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface PlanImageLayout {
  width: number;
  height: number;
  graphHeight: number;
  queryPanelY: number;
  queryPanelHeight: number;
  queryLines: string[];
}

interface PlanImageDocument {
  svg: SVGSVGElement;
  layout: PlanImageLayout;
}

export type SqlHighlightKind =
  | 'plain'
  | 'keyword'
  | 'string'
  | 'number'
  | 'comment'
  | 'identifier'
  | 'function'
  | 'operator';

export interface SqlHighlightToken {
  text: string;
  kind: SqlHighlightKind;
}

const SVG_NAMESPACE = 'http://www.w3.org/2000/svg';
const GRAPH_PADDING = 64;
const QUERY_PANEL_GAP = 24;
const QUERY_PANEL_PADDING = 32;
const QUERY_TITLE_HEIGHT = 28;
const QUERY_TITLE_GAP = 14;
const QUERY_FONT_SIZE = 18;
const QUERY_LINE_HEIGHT = 26;
const MIN_IMAGE_WIDTH = 1200;
const TARGET_RASTER_SCALE = 2;
const MAX_RASTER_DIMENSION = 16384;
const MAX_RASTER_PIXELS = 100_000_000;

const SQL_KEYWORDS = new Set([
  'ADD', 'ALL', 'ALTER', 'AND', 'ANY', 'AS', 'ASC', 'AUTHORIZATION', 'BACKUP',
  'BEGIN', 'BETWEEN', 'BREAK', 'BROWSE', 'BULK', 'BY', 'CASCADE', 'CASE', 'CHECK',
  'CHECKPOINT', 'CLOSE', 'CLUSTERED', 'COALESCE', 'COLLATE', 'COLUMN', 'COMMIT',
  'COMPUTE', 'CONSTRAINT', 'CONTAINS', 'CONTAINSTABLE', 'CONTINUE', 'CONVERT',
  'CREATE', 'CROSS', 'CURRENT', 'CURRENT_DATE', 'CURRENT_TIME', 'CURRENT_TIMESTAMP',
  'CURRENT_USER', 'CURSOR', 'DATABASE', 'DBCC', 'DEALLOCATE', 'DECLARE', 'DEFAULT',
  'DELETE', 'DENY', 'DESC', 'DISK', 'DISTINCT', 'DISTRIBUTED', 'DOUBLE', 'DROP',
  'DUMP', 'ELSE', 'END', 'ERRLVL', 'ESCAPE', 'EXCEPT', 'EXEC', 'EXECUTE', 'EXISTS',
  'EXIT', 'EXTERNAL', 'FETCH', 'FILE', 'FILLFACTOR', 'FOLLOWING', 'FOR', 'FOREIGN',
  'FREETEXT', 'FREETEXTTABLE', 'FROM', 'FULL', 'FUNCTION', 'GOTO', 'GRANT', 'GROUP',
  'HAVING', 'HOLDLOCK', 'IDENTITY', 'IDENTITY_INSERT', 'IDENTITYCOL', 'IF', 'IN',
  'INDEX', 'INNER', 'INSERT', 'INTERSECT', 'INTO', 'IS', 'JOIN', 'KEY', 'KILL',
  'LEFT', 'LIKE', 'LINENO', 'LOAD', 'MERGE', 'NATIONAL', 'NOCHECK', 'NONCLUSTERED',
  'NOT', 'NULL', 'NULLIF', 'OF', 'OFF', 'OFFSETS', 'ON', 'OPEN', 'OPENDATASOURCE',
  'OPENQUERY', 'OPENROWSET', 'OPENXML', 'OPTION', 'OR', 'ORDER', 'OUTER', 'OUTPUT',
  'OVER', 'PERCENT', 'PIVOT', 'PLAN', 'PRECISION', 'PRECEDING', 'PRIMARY', 'PRINT',
  'PROC', 'PROCEDURE', 'PUBLIC', 'RAISERROR', 'RANGE', 'READ', 'READTEXT',
  'RECONFIGURE', 'REFERENCES', 'REPLICATION', 'RESTORE', 'RESTRICT', 'RETURN',
  'REVERT', 'REVOKE', 'RIGHT', 'ROLLBACK', 'ROW', 'ROWS', 'RULE', 'SAVE', 'SCHEMA',
  'SELECT', 'SESSION_USER', 'SET', 'SETUSER', 'SHUTDOWN', 'SOME', 'STATISTICS',
  'SYSTEM_USER', 'TABLE', 'TABLESAMPLE', 'TEXTSIZE', 'THEN', 'THROW', 'TO', 'TOP',
  'TRANSACTION', 'TRIGGER', 'TRUNCATE', 'TRY_CONVERT', 'TSEQUAL', 'UNION', 'UNIQUE',
  'UNPIVOT', 'UPDATE', 'UPDATETEXT', 'USE', 'USER', 'VALUES', 'VARYING', 'VIEW',
  'WAITFOR', 'WHEN', 'WHERE', 'WHILE', 'WITH', 'WITHIN', 'WRITETEXT',
]);

const SQL_HIGHLIGHT_COLORS: Record<SqlHighlightKind, string> = {
  plain: '#cbd5e1',
  keyword: '#60a5fa',
  string: '#86efac',
  number: '#fbbf24',
  comment: '#64748b',
  identifier: '#67e8f9',
  function: '#c4b5fd',
  operator: '#f472b6',
};

const createSvgElement = <K extends keyof SVGElementTagNameMap>(
  name: K,
): SVGElementTagNameMap[K] => document.createElementNS(SVG_NAMESPACE, name);

export const wrapQueryForImage = (query: string, maxCharacters: number): string[] => {
  const limit = Math.max(1, Math.floor(maxCharacters));
  const normalized = query.replace(/\r\n?/g, '\n').trim();
  if (!normalized) return ['No query text available.'];

  const lines: string[] = [];
  for (const sourceLine of normalized.split('\n')) {
    let remaining = sourceLine.trimEnd();

    if (!remaining) {
      lines.push('');
      continue;
    }

    while (remaining.length > limit) {
      const candidate = remaining.slice(0, limit + 1);
      const whitespaceIndex = Math.max(candidate.lastIndexOf(' '), candidate.lastIndexOf('\t'));
      const breakIndex = whitespaceIndex > Math.floor(limit * 0.5) ? whitespaceIndex : limit;
      lines.push(remaining.slice(0, breakIndex).trimEnd());
      remaining = remaining.slice(breakIndex).trimStart();
    }

    lines.push(remaining);
  }

  return lines;
};

const appendHighlightToken = (
  tokens: SqlHighlightToken[],
  text: string,
  kind: SqlHighlightKind,
): void => {
  if (!text) return;
  const previous = tokens[tokens.length - 1];
  if (previous?.kind === kind) previous.text += text;
  else tokens.push({ text, kind });
};

export const highlightSqlForImage = (lines: string[]): SqlHighlightToken[][] => {
  let inBlockComment = false;
  let inString = false;

  return lines.map(line => {
    const tokens: SqlHighlightToken[] = [];
    let index = 0;

    while (index < line.length) {
      if (inBlockComment) {
        const end = line.indexOf('*/', index);
        if (end === -1) {
          appendHighlightToken(tokens, line.slice(index), 'comment');
          index = line.length;
          continue;
        }
        appendHighlightToken(tokens, line.slice(index, end + 2), 'comment');
        index = end + 2;
        inBlockComment = false;
        continue;
      }

      if (inString) {
        let end = index;
        while (end < line.length) {
          if (line[end] !== '\'') {
            end += 1;
            continue;
          }
          if (line[end + 1] === '\'') {
            end += 2;
            continue;
          }
          end += 1;
          inString = false;
          break;
        }
        appendHighlightToken(tokens, line.slice(index, end), 'string');
        index = end;
        continue;
      }

      if (line.startsWith('--', index)) {
        appendHighlightToken(tokens, line.slice(index), 'comment');
        break;
      }

      if (line.startsWith('/*', index)) {
        const end = line.indexOf('*/', index + 2);
        if (end === -1) {
          appendHighlightToken(tokens, line.slice(index), 'comment');
          inBlockComment = true;
          break;
        }
        appendHighlightToken(tokens, line.slice(index, end + 2), 'comment');
        index = end + 2;
        continue;
      }

      const character = line[index];
      const hasUnicodeStringPrefix = (character === 'N' || character === 'n') && line[index + 1] === '\'';
      if (character === '\'' || hasUnicodeStringPrefix) {
        let end = index + (hasUnicodeStringPrefix ? 2 : 1);
        let closed = false;
        while (end < line.length) {
          if (line[end] !== '\'') {
            end += 1;
            continue;
          }
          if (line[end + 1] === '\'') {
            end += 2;
            continue;
          }
          end += 1;
          closed = true;
          break;
        }
        appendHighlightToken(tokens, line.slice(index, end), 'string');
        inString = !closed;
        index = end;
        continue;
      }

      if (character === '[' || character === '"') {
        const closingCharacter = character === '[' ? ']' : '"';
        let end = index + 1;
        while (end < line.length) {
          if (line[end] !== closingCharacter) {
            end += 1;
            continue;
          }
          if (line[end + 1] === closingCharacter) {
            end += 2;
            continue;
          }
          end += 1;
          break;
        }
        appendHighlightToken(tokens, line.slice(index, end), 'identifier');
        index = end;
        continue;
      }

      if (/\d/.test(character)) {
        const match = line.slice(index).match(/^(?:0x[0-9a-f]+|\d+(?:\.\d+)?(?:e[+-]?\d+)?)/i);
        const value = match?.[0] ?? character;
        appendHighlightToken(tokens, value, 'number');
        index += value.length;
        continue;
      }

      if (/[A-Za-z_#@]/.test(character)) {
        const match = line.slice(index).match(/^[A-Za-z_#@][A-Za-z0-9_$#@]*/);
        const value = match?.[0] ?? character;
        const upperValue = value.toUpperCase();
        const followingCharacter = line.slice(index + value.length).trimStart()[0];
        let kind: SqlHighlightKind = 'plain';
        if (SQL_KEYWORDS.has(upperValue)) kind = 'keyword';
        else if (value.startsWith('@') || value.startsWith('#')) kind = 'identifier';
        else if (followingCharacter === '(') kind = 'function';
        appendHighlightToken(tokens, value, kind);
        index += value.length;
        continue;
      }

      if (/[+\-*/%=<>!|&^~]/.test(character)) {
        const match = line.slice(index).match(/^(?:<>|!=|<=|>=|!<|!>|\+=|-=|\*=|\/=|%=|&=|\^=|\|=|::|[+\-*/%=<>!|&^~])/);
        const value = match?.[0] ?? character;
        appendHighlightToken(tokens, value, 'operator');
        index += value.length;
        continue;
      }

      appendHighlightToken(tokens, character, 'plain');
      index += 1;
    }

    return tokens;
  });
};

export const calculatePlanImageLayout = (
  graphBounds: PlanGraphBounds,
  query: string,
): PlanImageLayout => {
  const width = Math.ceil(Math.max(MIN_IMAGE_WIDTH, graphBounds.width + GRAPH_PADDING * 2));
  const queryContentWidth = width - (GRAPH_PADDING + QUERY_PANEL_PADDING) * 2;
  const queryCharacterWidth = QUERY_FONT_SIZE * 0.62;
  const maxCharacters = Math.max(40, Math.floor(queryContentWidth / queryCharacterWidth));
  const queryLines = wrapQueryForImage(query, maxCharacters);
  const graphHeight = Math.ceil(graphBounds.height + GRAPH_PADDING * 2);
  const queryPanelY = graphHeight + QUERY_PANEL_GAP;
  const queryPanelHeight = QUERY_PANEL_PADDING * 2
    + QUERY_TITLE_HEIGHT
    + QUERY_TITLE_GAP
    + queryLines.length * QUERY_LINE_HEIGHT;

  return {
    width,
    height: Math.ceil(queryPanelY + queryPanelHeight + GRAPH_PADDING),
    graphHeight,
    queryPanelY,
    queryPanelHeight,
    queryLines,
  };
};

export const calculateRasterScale = (width: number, height: number): number => {
  const dimensionScale = Math.min(
    MAX_RASTER_DIMENSION / Math.max(1, width),
    MAX_RASTER_DIMENSION / Math.max(1, height),
  );
  const pixelScale = Math.sqrt(MAX_RASTER_PIXELS / Math.max(1, width * height));
  return Math.min(TARGET_RASTER_SCALE, dimensionScale, pixelScale);
};

export const createPlanImageDocument = (
  graphGroup: SVGGElement,
  graphBounds: PlanGraphBounds,
  query: string,
): PlanImageDocument => {
  const layout = calculatePlanImageLayout(graphBounds, query);
  const svg = createSvgElement('svg');
  svg.setAttribute('xmlns', SVG_NAMESPACE);
  svg.setAttribute('width', String(layout.width));
  svg.setAttribute('height', String(layout.height));
  svg.setAttribute('viewBox', `0 0 ${layout.width} ${layout.height}`);
  svg.setAttribute('font-family', 'Inter, ui-sans-serif, system-ui, sans-serif');
  svg.setAttribute('role', 'img');
  svg.setAttribute('aria-label', 'SQL Server execution plan with query text');

  const background = createSvgElement('rect');
  background.setAttribute('width', String(layout.width));
  background.setAttribute('height', String(layout.height));
  background.setAttribute('fill', '#1e293b');
  svg.appendChild(background);

  const graphClone = graphGroup.cloneNode(true) as SVGGElement;
  graphClone.removeAttribute('transform');
  graphClone.querySelectorAll('.edge-tooltip, .edge-hit-areas').forEach(element => element.remove());
  const graphX = (layout.width - graphBounds.width) / 2 - graphBounds.x;
  const graphY = GRAPH_PADDING - graphBounds.y;
  graphClone.setAttribute('transform', `translate(${graphX}, ${graphY})`);
  graphClone.setAttribute('pointer-events', 'none');
  svg.appendChild(graphClone);

  const queryPanel = createSvgElement('rect');
  queryPanel.setAttribute('x', String(GRAPH_PADDING));
  queryPanel.setAttribute('y', String(layout.queryPanelY));
  queryPanel.setAttribute('width', String(layout.width - GRAPH_PADDING * 2));
  queryPanel.setAttribute('height', String(layout.queryPanelHeight));
  queryPanel.setAttribute('rx', '12');
  queryPanel.setAttribute('fill', '#0f172a');
  queryPanel.setAttribute('stroke', '#334155');
  queryPanel.setAttribute('stroke-width', '2');
  svg.appendChild(queryPanel);

  const queryX = GRAPH_PADDING + QUERY_PANEL_PADDING;
  const titleY = layout.queryPanelY + QUERY_PANEL_PADDING + 20;
  const queryTitle = createSvgElement('text');
  queryTitle.setAttribute('x', String(queryX));
  queryTitle.setAttribute('y', String(titleY));
  queryTitle.setAttribute('fill', '#cbd5e1');
  queryTitle.setAttribute('font-size', '20');
  queryTitle.setAttribute('font-weight', '700');
  queryTitle.textContent = 'Query';
  svg.appendChild(queryTitle);

  const queryText = createSvgElement('text');
  queryText.setAttribute('x', String(queryX));
  queryText.setAttribute('y', String(titleY + QUERY_TITLE_GAP + QUERY_LINE_HEIGHT));
  queryText.setAttribute('fill', '#94a3b8');
  queryText.setAttribute('font-family', 'ui-monospace, SFMono-Regular, Consolas, monospace');
  queryText.setAttribute('font-size', String(QUERY_FONT_SIZE));
  queryText.setAttributeNS('http://www.w3.org/XML/1998/namespace', 'xml:space', 'preserve');

  highlightSqlForImage(layout.queryLines).forEach((tokens, lineIndex) => {
    const lineTokens = tokens.length > 0 ? tokens : [{ text: ' ', kind: 'plain' as const }];
    lineTokens.forEach((token, tokenIndex) => {
      const tspan = createSvgElement('tspan');
      if (tokenIndex === 0) {
        tspan.setAttribute('x', String(queryX));
        tspan.setAttribute('dy', lineIndex === 0 ? '0' : String(QUERY_LINE_HEIGHT));
      }
      tspan.setAttribute('fill', SQL_HIGHLIGHT_COLORS[token.kind]);
      tspan.textContent = token.text;
      queryText.appendChild(tspan);
    });
  });
  svg.appendChild(queryText);

  return { svg, layout };
};

const loadSvgImage = (url: string): Promise<HTMLImageElement> => new Promise((resolve, reject) => {
  const image = new Image();
  image.onload = () => resolve(image);
  image.onerror = () => reject(new Error('Could not render the execution plan image.'));
  image.src = url;
});

const canvasToBlob = (canvas: HTMLCanvasElement): Promise<Blob> => new Promise((resolve, reject) => {
  canvas.toBlob(blob => {
    if (blob) resolve(blob);
    else reject(new Error('Could not encode the execution plan image.'));
  }, 'image/png');
});

export const renderPlanAsHighDetailPng = async (
  graphGroup: SVGGElement,
  graphBounds: PlanGraphBounds,
  query: string,
): Promise<Blob> => {
  const { svg, layout } = createPlanImageDocument(graphGroup, graphBounds, query);
  const serializedSvg = new XMLSerializer().serializeToString(svg);
  const svgBlob = new Blob([serializedSvg], { type: 'image/svg+xml;charset=utf-8' });
  const svgUrl = URL.createObjectURL(svgBlob);

  try {
    const image = await loadSvgImage(svgUrl);
    const scale = calculateRasterScale(layout.width, layout.height);
    const canvas = document.createElement('canvas');
    canvas.width = Math.max(1, Math.ceil(layout.width * scale));
    canvas.height = Math.max(1, Math.ceil(layout.height * scale));
    const context = canvas.getContext('2d');
    if (!context) throw new Error('Image export is not supported by this system.');

    context.fillStyle = '#1e293b';
    context.fillRect(0, 0, canvas.width, canvas.height);
    context.scale(scale, scale);
    context.drawImage(image, 0, 0, layout.width, layout.height);

    return await canvasToBlob(canvas);
  } finally {
    URL.revokeObjectURL(svgUrl);
  }
};

export const copyPlanPngToClipboard = async (pngBlob: Promise<Blob>): Promise<void> => {
  if (!navigator.clipboard?.write || typeof ClipboardItem === 'undefined') {
    throw new Error('Image clipboard access is not supported by this system.');
  }

  await navigator.clipboard.write([
    new ClipboardItem({ 'image/png': pngBlob }),
  ]);
};
