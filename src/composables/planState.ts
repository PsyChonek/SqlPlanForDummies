import { reactive } from 'vue';

export interface Capsule {
  keystone: string;
  nomenclature: string;
  magnitude: number;
  temporalCost: number;
  densityFactor: number;
  offspring: Capsule[];
  annotations: Record<string, any>;
}

const centralState = reactive({
  rootCapsule: null as Capsule | null,
  inspectedCapsule: null as Capsule | null,
  revision: 0  // Incremented to trigger Vue reactivity when state changes
});

export const usePlanState = () => {
  const injectRootCapsule = (capsule: Capsule) => {
    centralState.rootCapsule = capsule;
    centralState.inspectedCapsule = null;
    centralState.revision++;
  };

  const setInspectedCapsule = (capsule: Capsule | null) => {
    centralState.inspectedCapsule = capsule;
    centralState.revision++;
  };

  return {
    centralState,
    injectRootCapsule,
    setInspectedCapsule
  };
};
