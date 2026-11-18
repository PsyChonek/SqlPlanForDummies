import { useSqlEditorState } from './useSqlEditorState';

describe('useSqlEditorState', () => {
  let editor: ReturnType<typeof useSqlEditorState>;

  beforeEach(() => {
    editor = useSqlEditorState();
    // Reset the shared singleton to a single empty tab
    editor.tabs.splice(1);
    editor.tabs[0].content = '';
    editor.activeTabId.value = editor.tabs[0].id;
  });

  describe('openInEditor', () => {
    it('fills the active tab when it is empty', () => {
      editor.openInEditor('SELECT 1');

      expect(editor.tabs).toHaveLength(1);
      expect(editor.tabs[0].content).toBe('SELECT 1');
    });

    it('opens a new active tab when the current one has content', () => {
      editor.tabs[0].content = 'SELECT draft';

      editor.openInEditor('SELECT 2');

      expect(editor.tabs).toHaveLength(2);
      expect(editor.tabs[0].content).toBe('SELECT draft');
      expect(editor.getTab(editor.activeTabId.value)?.content).toBe('SELECT 2');
    });
  });
});
