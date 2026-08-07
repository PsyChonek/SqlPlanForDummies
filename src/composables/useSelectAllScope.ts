import { onMounted, onBeforeUnmount } from 'vue';

// Scopes Ctrl+A to the panel the user last clicked in (marked with
// data-select-scope) instead of selecting the whole app. Inputs, textareas,
// contenteditable elements, and CodeMirror keep their native select-all.
export function useSelectAllScope() {
  let lastPointerTarget: HTMLElement | null = null;

  const onPointerDown = (evt: PointerEvent) => {
    lastPointerTarget = evt.target instanceof HTMLElement ? evt.target : null;
  };

  const onKeyDown = (evt: KeyboardEvent) => {
    if (!(evt.ctrlKey || evt.metaKey) || evt.altKey || evt.shiftKey) return;
    if (evt.key.toLowerCase() !== 'a') return;

    const active = document.activeElement instanceof HTMLElement && document.activeElement !== document.body
      ? document.activeElement
      : null;
    const reference = active ?? lastPointerTarget;
    if (!reference || !document.contains(reference)) return;

    if (reference.closest('input, textarea, [contenteditable="true"], .cm-editor')) return;

    evt.preventDefault();
    const selection = window.getSelection();
    if (!selection) return;
    selection.removeAllRanges();

    const scope = reference.closest('[data-select-scope]');
    if (scope) {
      const range = document.createRange();
      range.selectNodeContents(scope);
      selection.addRange(range);
    }
  };

  onMounted(() => {
    document.addEventListener('pointerdown', onPointerDown, true);
    document.addEventListener('keydown', onKeyDown);
  });

  onBeforeUnmount(() => {
    document.removeEventListener('pointerdown', onPointerDown, true);
    document.removeEventListener('keydown', onKeyDown);
  });
}
