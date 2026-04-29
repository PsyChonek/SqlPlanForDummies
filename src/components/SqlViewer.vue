<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue';
import { EditorView } from '@codemirror/view';
import { EditorState } from '@codemirror/state';
import { sql, MSSQL } from '@codemirror/lang-sql';
import { oneDark } from '@codemirror/theme-one-dark';

const props = defineProps<{ text: string }>();

const containerRef = ref<HTMLDivElement | null>(null);
let view: EditorView | null = null;

onMounted(() => {
  if (!containerRef.value) return;

  view = new EditorView({
    state: EditorState.create({
      doc: props.text,
      extensions: [
        sql({ dialect: MSSQL }),
        oneDark,
        EditorState.readOnly.of(true),
        EditorView.editable.of(false),
        EditorView.theme({
          '&': { height: '100%', fontSize: '13px' },
          '.cm-scroller': { overflow: 'auto' },
          '.cm-content': { fontFamily: "'Cascadia Code', 'Fira Code', monospace", padding: '16px' },
          '.cm-focused': { outline: 'none' },
          '.cm-gutters': { display: 'none' },
        }),
      ],
    }),
    parent: containerRef.value,
  });
});

watch(() => props.text, (newText) => {
  if (!view) return;
  view.dispatch({
    changes: { from: 0, to: view.state.doc.length, insert: newText },
  });
});

onUnmounted(() => {
  view?.destroy();
  view = null;
});
</script>

<template>
  <div ref="containerRef" class="h-full" />
</template>
