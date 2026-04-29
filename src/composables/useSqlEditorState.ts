import { reactive, ref } from 'vue';
import type { PlanType } from './useQueryExecution';

export interface EditorTabState {
  id: string;
  title: string;
  content: string;
}

let tabCounter = 1;

const tabs = reactive<EditorTabState[]>([
  { id: crypto.randomUUID(), title: 'Query 1', content: '' },
]);

const activeTabId = ref(tabs[0].id);
const planType = ref<PlanType>('Estimated');

export const useSqlEditorState = () => {
  const getTab = (tabId: string) => tabs.find((t) => t.id === tabId);

  const setContent = (tabId: string, content: string) => {
    const tab = getTab(tabId);
    if (tab) tab.content = content;
  };

  const addTab = () => {
    tabCounter++;
    const newTab: EditorTabState = {
      id: crypto.randomUUID(),
      title: `Query ${tabCounter}`,
      content: '',
    };
    tabs.push(newTab);
    activeTabId.value = newTab.id;
  };

  const closeTab = (tabId: string) => {
    if (tabs.length <= 1) return;
    const idx = tabs.findIndex((t) => t.id === tabId);
    tabs.splice(idx, 1);
    if (activeTabId.value === tabId) {
      activeTabId.value = tabs[Math.min(idx, tabs.length - 1)].id;
    }
  };

  return { tabs, activeTabId, planType, getTab, setContent, addTab, closeTab };
};
