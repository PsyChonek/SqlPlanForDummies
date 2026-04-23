<script setup lang="ts">
import { ref, watch, computed, onMounted, onUnmounted } from 'vue';
import { useXelState } from '../../composables/useXelState';

const { state, setFilter, clearFilter, hasActiveFilters, eventTypes } = useXelState();

const searchText = ref('');
let debounceTimer: ReturnType<typeof setTimeout> | null = null;
let suppressSync = false;

watch(searchText, (val) => {
  if (debounceTimer) clearTimeout(debounceTimer);
  suppressSync = true;
  debounceTimer = setTimeout(() => {
    setFilter({ textSearch: val.length > 0 ? val : null });
    suppressSync = false;
  }, 200);
});

// Sync back when filters are set programmatically (e.g. from event details)
watch(() => state.filter.textSearch, (val) => {
  if (!suppressSync) {
    // Cancel any pending debounce to prevent it from overwriting the programmatic value
    if (debounceTimer) { clearTimeout(debounceTimer); debounceTimer = null; }
    searchText.value = val ?? '';
  }
});
watch(() => state.filter.timeFrom, (val) => { timeFrom.value = val ?? ''; });
watch(() => state.filter.timeTo, (val) => { timeTo.value = val ?? ''; });

const clearSearch = () => {
  searchText.value = '';
  setFilter({ textSearch: null });
};

// Toggle helpers
const isErrorsActive = computed(() => state.filter.errorsOnly);
const isDeadlocksActive = computed(() => state.filter.deadlocksOnly);
const isLocksActive = computed(() => state.filter.eventNames.some(n => n.startsWith('lock_') || n === 'blocked_process_report'));
const isSlowActive = computed(() => state.filter.minDurationUs !== null);
const isWaitsActive = computed(() => state.filter.eventNames.includes('wait_completed'));

const toggleErrors = () => {
  setFilter({ errorsOnly: !state.filter.errorsOnly });
};

const toggleDeadlocks = () => {
  setFilter({ deadlocksOnly: !state.filter.deadlocksOnly });
};

const toggleLocks = () => {
  if (isLocksActive.value) {
    setFilter({ eventNames: [] });
  } else {
    setFilter({ eventNames: ['lock_acquired', 'lock_timeout_greater_than_0', 'lock_escalation', 'blocked_process_report'] });
  }
};

const toggleWaits = () => {
  if (isWaitsActive.value) {
    setFilter({ eventNames: [] });
  } else {
    setFilter({ eventNames: ['wait_completed'] });
  }
};

const showSlowPicker = ref(false);
const slowOptions = [
  { label: '> 100ms', value: 100_000 },
  { label: '> 500ms', value: 500_000 },
  { label: '> 1s', value: 1_000_000 },
  { label: '> 5s', value: 5_000_000 },
  { label: '> 10s', value: 10_000_000 },
  { label: '> 30s', value: 30_000_000 },
];

const setSlowThreshold = (value: number) => {
  setFilter({ minDurationUs: state.filter.minDurationUs === value ? null : value });
  showSlowPicker.value = false;
};

const currentSlowLabel = computed(() => {
  const match = slowOptions.find(o => o.value === state.filter.minDurationUs);
  return match ? match.label : null;
});

// Date range filter — custom picker
const timeFrom = ref('');
const timeTo = ref('');
const showDatePicker = ref<'from' | 'to' | null>(null);
const datePickerRef = ref<HTMLDivElement | null>(null);

// Picker state
const pickerYear = ref(new Date().getFullYear());
const pickerMonth = ref(new Date().getMonth());
const pickerHour = ref(0);
const pickerMinute = ref(0);
const pickerSecond = ref(0);

const monthNames = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
const dayNames = ['Su', 'Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa'];

const daysInMonth = computed(() => new Date(pickerYear.value, pickerMonth.value + 1, 0).getDate());
const firstDayOfWeek = computed(() => new Date(pickerYear.value, pickerMonth.value, 1).getDay());

const calendarDays = computed(() => {
  const days: (number | null)[] = [];
  for (let i = 0; i < firstDayOfWeek.value; i++) days.push(null);
  for (let d = 1; d <= daysInMonth.value; d++) days.push(d);
  return days;
});

const openDatePicker = (which: 'from' | 'to') => {
  const current = which === 'from' ? timeFrom.value : timeTo.value;
  if (current) {
    const d = new Date(current);
    pickerYear.value = d.getFullYear();
    pickerMonth.value = d.getMonth();
    pickerHour.value = d.getHours();
    pickerMinute.value = d.getMinutes();
    pickerSecond.value = d.getSeconds();
  } else {
    const now = new Date();
    pickerYear.value = now.getFullYear();
    pickerMonth.value = now.getMonth();
    pickerHour.value = 0;
    pickerMinute.value = 0;
    pickerSecond.value = 0;
  }
  showDatePicker.value = which;
};

const prevMonth = () => {
  if (pickerMonth.value === 0) { pickerMonth.value = 11; pickerYear.value--; }
  else pickerMonth.value--;
};
const nextMonth = () => {
  if (pickerMonth.value === 11) { pickerMonth.value = 0; pickerYear.value++; }
  else pickerMonth.value++;
};

const selectDay = (day: number) => {
  const pad = (n: number) => String(n).padStart(2, '0');
  const val = `${pickerYear.value}-${pad(pickerMonth.value + 1)}-${pad(day)}T${pad(pickerHour.value)}:${pad(pickerMinute.value)}:${pad(pickerSecond.value)}`;
  if (showDatePicker.value === 'from') {
    timeFrom.value = val;
    setFilter({ timeFrom: val });
  } else {
    timeTo.value = val;
    setFilter({ timeTo: val });
  }
  showDatePicker.value = null;
};

const formatDisplay = (val: string) => {
  if (!val) return '';
  const d = new Date(val);
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
};

const clearDateFilter = (which: 'from' | 'to') => {
  if (which === 'from') { timeFrom.value = ''; setFilter({ timeFrom: null }); }
  else { timeTo.value = ''; setFilter({ timeTo: null }); }
};

const isSelectedDay = (day: number) => {
  const current = showDatePicker.value === 'from' ? timeFrom.value : timeTo.value;
  if (!current) return false;
  const d = new Date(current);
  return d.getFullYear() === pickerYear.value && d.getMonth() === pickerMonth.value && d.getDate() === day;
};

// Event type dropdown
const showEventTypes = ref(false);
const slowPickerRef = ref<HTMLDivElement | null>(null);
const eventTypesRef = ref<HTMLDivElement | null>(null);

// Close all dropdowns on outside click
const onDocClick = (e: MouseEvent) => {
  const target = e.target as Node;
  if (datePickerRef.value && !datePickerRef.value.contains(target)) {
    showDatePicker.value = null;
  }
  if (slowPickerRef.value && !slowPickerRef.value.contains(target)) {
    showSlowPicker.value = false;
  }
  if (eventTypesRef.value && !eventTypesRef.value.contains(target)) {
    showEventTypes.value = false;
  }
};
onMounted(() => document.addEventListener('mousedown', onDocClick));
onUnmounted(() => document.removeEventListener('mousedown', onDocClick));
const toggleEventType = (name: string) => {
  const current = [...state.filter.eventNames];
  const idx = current.indexOf(name);
  if (idx >= 0) {
    current.splice(idx, 1);
  } else {
    current.push(name);
  }
  setFilter({ eventNames: current });
};
</script>

<template>
  <div class="shrink-0 flex flex-wrap items-center gap-1.5 px-3 py-2 bg-slate-750 border-b border-slate-600">
    <!-- Search -->
    <div class="relative flex-1 min-w-[200px] max-w-sm">
      <i class="fa-solid fa-search absolute left-2.5 top-1/2 -translate-y-1/2 text-slate-500 text-xs"></i>
      <input
        v-model="searchText"
        type="text"
        placeholder="Search... e.g. session_id:64 &quot;exact phrase&quot; || (event_name:rpc lock)"
        class="w-full pl-8 pr-8 py-1 bg-slate-700 border border-slate-600 rounded-lg text-sm text-slate-200 placeholder-slate-500 focus:outline-none focus:border-indigo-500"
      />
      <button
        v-if="searchText"
        @click="clearSearch"
        class="absolute right-2 top-1/2 -translate-y-1/2 text-slate-500 hover:text-slate-300"
      >
        <i class="fa-solid fa-xmark text-xs"></i>
      </button>
    </div>

    <!-- Quick filters -->
    <button
      @click="toggleErrors"
      class="px-2 py-1 text-xs rounded-lg border transition-colors"
      :class="isErrorsActive
        ? 'bg-red-900/30 border-red-700/50 text-red-300'
        : 'bg-slate-700 border-slate-600 text-slate-400 hover:text-slate-300'"
    >
      <i class="fa-solid fa-circle-exclamation mr-1"></i>Errors
    </button>

    <button
      @click="toggleDeadlocks"
      class="px-2 py-1 text-xs rounded-lg border transition-colors"
      :class="isDeadlocksActive
        ? 'bg-red-900/30 border-red-700/50 text-red-300'
        : 'bg-slate-700 border-slate-600 text-slate-400 hover:text-slate-300'"
    >
      <i class="fa-solid fa-skull-crossbones mr-1"></i>Deadlocks
    </button>

    <button
      @click="toggleLocks"
      class="px-2 py-1 text-xs rounded-lg border transition-colors"
      :class="isLocksActive
        ? 'bg-yellow-900/30 border-yellow-700/50 text-yellow-300'
        : 'bg-slate-700 border-slate-600 text-slate-400 hover:text-slate-300'"
    >
      <i class="fa-solid fa-lock mr-1"></i>Locks
    </button>

    <button
      @click="toggleWaits"
      class="px-2 py-1 text-xs rounded-lg border transition-colors"
      :class="isWaitsActive
        ? 'bg-orange-900/30 border-orange-700/50 text-orange-300'
        : 'bg-slate-700 border-slate-600 text-slate-400 hover:text-slate-300'"
    >
      <i class="fa-solid fa-clock mr-1"></i>Waits
    </button>

    <div ref="slowPickerRef" class="relative">
      <button
        @click="showSlowPicker = !showSlowPicker"
        class="px-2 py-1 text-xs rounded-lg border transition-colors"
        :class="isSlowActive && state.filter.minDurationUs !== 29_000_000
          ? 'bg-orange-900/30 border-orange-700/50 text-orange-300'
          : 'bg-slate-700 border-slate-600 text-slate-400 hover:text-slate-300'"
      >
        <i class="fa-solid fa-gauge-high mr-1"></i>Slow
        <span v-if="currentSlowLabel" class="ml-0.5 text-orange-400">{{ currentSlowLabel }}</span>
        <i class="fa-solid fa-caret-down ml-1 text-[10px]"></i>
      </button>
      <div
        v-if="showSlowPicker"
        class="absolute top-full left-0 mt-1 z-50 bg-slate-800 border border-slate-600 rounded-lg shadow-xl p-1 min-w-[120px]"
      >
        <button
          v-for="opt in slowOptions"
          :key="opt.value"
          @click="setSlowThreshold(opt.value)"
          class="block w-full text-left px-2 py-1 text-xs rounded hover:bg-slate-700 transition-colors"
          :class="state.filter.minDurationUs === opt.value ? 'text-orange-300' : 'text-slate-400'"
        >
          {{ opt.label }}
        </button>
      </div>
    </div>

    <!-- Date range -->
    <div ref="datePickerRef" class="relative flex items-center gap-1.5">
      <!-- From button -->
      <button
        @click="openDatePicker('from')"
        class="px-2 py-1 text-xs rounded-lg border transition-colors flex items-center gap-1"
        :class="state.filter.timeFrom
          ? 'bg-indigo-900/30 border-indigo-600/50 text-indigo-300'
          : 'bg-slate-700 border-slate-600 text-slate-400 hover:text-slate-300'"
      >
        <i class="fa-solid fa-calendar text-[10px]"></i>
        <span v-if="state.filter.timeFrom">{{ formatDisplay(timeFrom || state.filter.timeFrom) }}</span>
        <span v-else>From</span>
        <button
          v-if="state.filter.timeFrom"
          @click.stop="clearDateFilter('from')"
          class="ml-0.5 text-slate-500 hover:text-slate-200"
        >
          <i class="fa-solid fa-xmark text-[9px]"></i>
        </button>
      </button>

      <span class="text-slate-600 text-xs">-</span>

      <!-- To button -->
      <button
        @click="openDatePicker('to')"
        class="px-2 py-1 text-xs rounded-lg border transition-colors flex items-center gap-1"
        :class="state.filter.timeTo
          ? 'bg-indigo-900/30 border-indigo-600/50 text-indigo-300'
          : 'bg-slate-700 border-slate-600 text-slate-400 hover:text-slate-300'"
      >
        <i class="fa-solid fa-calendar-check text-[10px]"></i>
        <span v-if="state.filter.timeTo">{{ formatDisplay(timeTo || state.filter.timeTo) }}</span>
        <span v-else>To</span>
        <button
          v-if="state.filter.timeTo"
          @click.stop="clearDateFilter('to')"
          class="ml-0.5 text-slate-500 hover:text-slate-200"
        >
          <i class="fa-solid fa-xmark text-[9px]"></i>
        </button>
      </button>

      <!-- Custom date picker dropdown -->
      <div
        v-if="showDatePicker"
        class="absolute top-full mt-1 z-50 bg-slate-800 border border-slate-600 rounded-lg shadow-xl p-3 select-none"
        style="min-width: 260px;"
      >
        <!-- Month/year nav -->
        <div class="flex items-center justify-between mb-2">
          <button @click="prevMonth" class="p-1 text-slate-400 hover:text-slate-200 transition-colors">
            <i class="fa-solid fa-chevron-left text-xs"></i>
          </button>
          <span class="text-sm text-slate-200 font-medium">{{ monthNames[pickerMonth] }} {{ pickerYear }}</span>
          <button @click="nextMonth" class="p-1 text-slate-400 hover:text-slate-200 transition-colors">
            <i class="fa-solid fa-chevron-right text-xs"></i>
          </button>
        </div>

        <!-- Day headers -->
        <div class="grid grid-cols-7 gap-0 mb-1">
          <div v-for="d in dayNames" :key="d" class="text-center text-[10px] text-slate-500 py-0.5">{{ d }}</div>
        </div>

        <!-- Day grid -->
        <div class="grid grid-cols-7 gap-0">
          <div v-for="(day, i) in calendarDays" :key="i" class="text-center">
            <button
              v-if="day"
              @click="selectDay(day)"
              class="w-7 h-7 text-xs rounded-full transition-colors"
              :class="isSelectedDay(day)
                ? 'bg-indigo-600 text-white'
                : 'text-slate-300 hover:bg-slate-700'"
            >
              {{ day }}
            </button>
          </div>
        </div>

        <!-- Time inputs -->
        <div class="flex items-center justify-center gap-1 mt-3 pt-2 border-t border-slate-700">
          <i class="fa-solid fa-clock text-slate-500 text-[10px] mr-1"></i>
          <input
            v-model.number="pickerHour"
            type="number" min="0" max="23"
            class="w-10 px-1 py-0.5 bg-slate-700 border border-slate-600 rounded text-xs text-slate-200 text-center focus:outline-none focus:border-indigo-500"
          />
          <span class="text-slate-500 text-xs">:</span>
          <input
            v-model.number="pickerMinute"
            type="number" min="0" max="59"
            class="w-10 px-1 py-0.5 bg-slate-700 border border-slate-600 rounded text-xs text-slate-200 text-center focus:outline-none focus:border-indigo-500"
          />
          <span class="text-slate-500 text-xs">:</span>
          <input
            v-model.number="pickerSecond"
            type="number" min="0" max="59"
            class="w-10 px-1 py-0.5 bg-slate-700 border border-slate-600 rounded text-xs text-slate-200 text-center focus:outline-none focus:border-indigo-500"
          />
        </div>
      </div>
    </div>

    <!-- Event type picker -->
    <div ref="eventTypesRef" class="relative">
      <button
        @click="showEventTypes = !showEventTypes"
        class="px-2 py-1 text-xs rounded-lg border transition-colors"
        :class="state.filter.eventNames.length > 0
          ? 'bg-indigo-900/30 border-indigo-700/50 text-indigo-300'
          : 'bg-slate-700 border-slate-600 text-slate-400 hover:text-slate-300'"
      >
        <i class="fa-solid fa-filter mr-1"></i>Types
        <span v-if="state.filter.eventNames.length > 0" class="ml-1 text-indigo-400">({{ state.filter.eventNames.filter(n => eventTypes.some(et => et.name === n)).length }})</span>
      </button>
      <div
        v-if="showEventTypes"
        class="absolute top-full left-0 mt-1 z-50 bg-slate-800 border border-slate-600 rounded-lg shadow-xl p-2 min-w-[200px] max-h-48 overflow-auto"
      >
        <button
          v-for="et in eventTypes"
          :key="et.name"
          @click="toggleEventType(et.name)"
          class="flex items-center justify-between w-full px-2 py-1 text-xs rounded hover:bg-slate-700 transition-colors"
          :class="state.filter.eventNames.includes(et.name) ? 'text-indigo-300' : 'text-slate-400'"
        >
          <span>{{ et.name }}</span>
          <span class="text-slate-500 ml-2">{{ et.count.toLocaleString() }}</span>
        </button>
      </div>
    </div>

    <!-- Clear filters -->
    <button
      v-if="hasActiveFilters"
      @click="clearFilter(); searchText = ''; timeFrom = ''; timeTo = ''"
      class="px-2 py-1 text-xs rounded-lg bg-slate-700 border border-slate-600 text-slate-400 hover:text-red-400 transition-colors"
    >
      <i class="fa-solid fa-filter-circle-xmark mr-1"></i>Clear
    </button>
  </div>
</template>

<style scoped>
/* Hide number input spinners in time fields */
input[type="number"]::-webkit-inner-spin-button,
input[type="number"]::-webkit-outer-spin-button {
  -webkit-appearance: none;
  margin: 0;
}
input[type="number"] {
  -moz-appearance: textfield;
}
</style>
