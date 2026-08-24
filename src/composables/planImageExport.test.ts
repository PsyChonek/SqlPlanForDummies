import {
  calculatePlanImageLayout,
  calculateRasterScale,
  copyPlanPngToClipboard,
  createPlanImageDocument,
  highlightSqlForImage,
  wrapQueryForImage,
} from './planImageExport';

const SVG_NAMESPACE = 'http://www.w3.org/2000/svg';

describe('plan image export', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('wraps the complete query without truncating it', () => {
    const query = 'SELECT EmployeeId, FirstName, LastName FROM Employees WHERE DepartmentId = 42 ORDER BY LastName';
    const lines = wrapQueryForImage(query, 28);

    expect(lines.length).toBeGreaterThan(1);
    expect(lines.every(line => line.length <= 28)).toBe(true);
    expect(lines.join(' ')).toBe(query);
  });

  it('preserves explicit query line breaks', () => {
    const lines = wrapQueryForImage('SELECT *\nFROM Employees\n\nWHERE Active = 1', 80);

    expect(lines).toEqual(['SELECT *', 'FROM Employees', '', 'WHERE Active = 1']);
  });

  it('highlights SQL syntax by token type', () => {
    const [tokens] = highlightSqlForImage([
      "SELECT [Name], COUNT(*) FROM Employees WHERE Id >= 42 AND Name = N'Ada' -- active employee",
    ]);

    expect(tokens).toContainEqual({ text: 'SELECT', kind: 'keyword' });
    expect(tokens).toContainEqual({ text: '[Name]', kind: 'identifier' });
    expect(tokens).toContainEqual({ text: 'COUNT', kind: 'function' });
    expect(tokens).toContainEqual({ text: '>=', kind: 'operator' });
    expect(tokens).toContainEqual({ text: '42', kind: 'number' });
    expect(tokens).toContainEqual({ text: "N'Ada'", kind: 'string' });
    expect(tokens).toContainEqual({ text: '-- active employee', kind: 'comment' });
  });

  it('keeps multiline comments highlighted across rendered lines', () => {
    const lines = highlightSqlForImage(['SELECT /* first line', 'second line */ Id', 'FROM Employees']);

    expect(lines[0][lines[0].length - 1]).toEqual({ text: '/* first line', kind: 'comment' });
    expect(lines[1][0]).toEqual({ text: 'second line */', kind: 'comment' });
    expect(lines[2][0]).toEqual({ text: 'FROM', kind: 'keyword' });
  });

  it('creates enough space for every graph node and the query panel', () => {
    const layout = calculatePlanImageLayout(
      { x: -100, y: -50, width: 1800, height: 700 },
      'SELECT * FROM Employees',
    );

    expect(layout.width).toBeGreaterThanOrEqual(1800 + 128);
    expect(layout.graphHeight).toBe(700 + 128);
    expect(layout.queryPanelY).toBeGreaterThan(layout.graphHeight);
    expect(layout.height).toBeGreaterThan(layout.queryPanelY + layout.queryPanelHeight);
  });

  it('clones the full graph and removes interactive-only layers', () => {
    const graph = document.createElementNS(SVG_NAMESPACE, 'g');
    graph.setAttribute('transform', 'translate(20, 30) scale(0.5)');

    const nodes = document.createElementNS(SVG_NAMESPACE, 'g');
    nodes.setAttribute('class', 'nodes');
    for (const nodeId of ['1', '2']) {
      const node = document.createElementNS(SVG_NAMESPACE, 'rect');
      node.setAttribute('data-node-id', nodeId);
      nodes.appendChild(node);
    }
    graph.appendChild(nodes);

    const tooltip = document.createElementNS(SVG_NAMESPACE, 'g');
    tooltip.setAttribute('class', 'edge-tooltip');
    graph.appendChild(tooltip);

    const hitAreas = document.createElementNS(SVG_NAMESPACE, 'g');
    hitAreas.setAttribute('class', 'edge-hit-areas');
    graph.appendChild(hitAreas);

    const { svg } = createPlanImageDocument(
      graph,
      { x: 0, y: 0, width: 800, height: 400 },
      'SELECT <all columns> FROM Employees',
    );

    expect(svg.querySelectorAll('[data-node-id]')).toHaveLength(2);
    expect(svg.querySelector('.edge-tooltip')).toBeNull();
    expect(svg.querySelector('.edge-hit-areas')).toBeNull();
    expect(svg.textContent).toContain('SELECT <all columns> FROM Employees');
    expect(svg.querySelectorAll('text tspan[fill]').length).toBeGreaterThan(1);
    expect(svg.querySelector('.nodes')?.parentElement?.getAttribute('transform')).not.toContain('scale');
  });

  it('writes the generated PNG to the image clipboard', async () => {
    class ClipboardItemMock {
      constructor(readonly data: Record<string, Blob | Promise<Blob>>) {}
    }

    const write = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal('navigator', { clipboard: { write } });
    vi.stubGlobal('ClipboardItem', ClipboardItemMock);
    const pngBlob = Promise.resolve(new Blob(['png'], { type: 'image/png' }));

    await copyPlanPngToClipboard(pngBlob);

    expect(write).toHaveBeenCalledWith([expect.any(ClipboardItemMock)]);
  });

  it('uses 2x detail for normal plans and caps oversized canvases', () => {
    expect(calculateRasterScale(2000, 1000)).toBe(2);
    expect(calculateRasterScale(20000, 1000)).toBeLessThan(1);
    expect(calculateRasterScale(12000, 12000)).toBeLessThan(1);
  });
});
