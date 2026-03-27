import { ref, onUnmounted } from 'vue';

export interface ResizePanelOptions {
  /** Initial size in pixels */
  initial: number;
  /** Below this size the panel collapses (px) */
  collapseAt?: number;
  /** Direction: which edge the handle sits on */
  direction: 'left' | 'right' | 'top' | 'bottom';
  /** Optional callback returning the max allowed size (called during drag) */
  getMaxSize?: () => number;
}

export function useResizePanel(options: ResizePanelOptions) {
  const size = ref(options.initial);
  const collapsed = ref(false);
  const dragging = ref(false);
  const restoreSize = ref(options.initial);

  let startPos = 0;
  let startSize = 0;

  const collapseThreshold = options.collapseAt ?? 50;

  const onPointerDown = (e: PointerEvent) => {
    e.preventDefault();
    dragging.value = true;
    startPos = options.direction === 'left' || options.direction === 'right' ? e.clientX : e.clientY;
    startSize = collapsed.value ? 0 : size.value;

    document.body.style.cursor = (options.direction === 'left' || options.direction === 'right') ? 'col-resize' : 'row-resize';
    document.body.style.userSelect = 'none';

    document.addEventListener('pointermove', onPointerMove);
    document.addEventListener('pointerup', onPointerUp);
  };

  const onPointerMove = (e: PointerEvent) => {
    const isHorizontal = options.direction === 'left' || options.direction === 'right';
    const currentPos = isHorizontal ? e.clientX : e.clientY;
    let delta = currentPos - startPos;

    // Invert delta for panels on the right/bottom (dragging left = grow)
    if (options.direction === 'right' || options.direction === 'bottom') {
      delta = -delta;
    }

    let newSize = startSize + delta;

    if (newSize < collapseThreshold) {
      if (!collapsed.value) {
        restoreSize.value = startSize > collapseThreshold ? startSize : options.initial;
      }
      collapsed.value = true;
      size.value = 0;
    } else {
      collapsed.value = false;
      const max = options.getMaxSize ? options.getMaxSize() : Infinity;
      size.value = Math.min(max, Math.max(1, newSize));
    }
  };

  const onPointerUp = () => {
    dragging.value = false;
    document.body.style.cursor = '';
    document.body.style.userSelect = '';
    document.removeEventListener('pointermove', onPointerMove);
    document.removeEventListener('pointerup', onPointerUp);
  };

  /** Double-click to toggle collapse/restore */
  const onDoubleClick = () => {
    if (collapsed.value) {
      collapsed.value = false;
      size.value = restoreSize.value;
    } else {
      restoreSize.value = size.value;
      collapsed.value = true;
      size.value = 0;
    }
  };

  onUnmounted(() => {
    document.removeEventListener('pointermove', onPointerMove);
    document.removeEventListener('pointerup', onPointerUp);
  });

  return { size, collapsed, dragging, onPointerDown, onDoubleClick };
}
