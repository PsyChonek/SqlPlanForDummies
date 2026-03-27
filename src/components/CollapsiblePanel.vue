<script setup lang="ts">
import { ref } from 'vue';

const props = withDefaults(defineProps<{
  title: string;
  icon?: string;
  iconColor?: string;
  badge?: string | number;
  collapsed?: boolean;
  /** Extra classes for the outer wrapper */
  panelClass?: string;
  /** Extra classes for the header row */
  headerClass?: string;
}>(), {
  collapsed: false,
  iconColor: 'text-slate-400',
});

const isCollapsed = ref(props.collapsed);
const toggle = () => { isCollapsed.value = !isCollapsed.value; };

defineExpose({ isCollapsed, toggle });
</script>

<template>
  <div :class="panelClass">
    <!-- Clickable header -->
    <button
      type="button"
      class="w-full flex items-center gap-2 cursor-pointer select-none group"
      :class="headerClass"
      @click="toggle"
    >
      <i
        class="fa-solid fa-chevron-right text-[10px] text-slate-500 transition-transform duration-200"
        :class="{ 'rotate-90': !isCollapsed }"
      ></i>
      <i v-if="icon" :class="['fa-solid', icon, iconColor]" class="text-xs"></i>
      <span class="flex-1 text-left text-sm font-semibold text-slate-300 group-hover:text-slate-100 transition-colors">
        {{ title }}
      </span>
      <span v-if="badge !== undefined && badge !== ''" class="text-xs text-slate-500">{{ badge }}</span>
      <slot name="header-right" />
    </button>

    <!-- Collapsible content -->
    <div
      class="overflow-hidden transition-all duration-200"
      :class="isCollapsed ? 'max-h-0 opacity-0' : 'max-h-[5000px] opacity-100'"
    >
      <div class="pt-1.5 pl-5">
        <slot />
      </div>
    </div>
  </div>
</template>
