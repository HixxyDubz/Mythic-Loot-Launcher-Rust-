import { useCallback, useEffect, useRef } from "react";

// A pending response cannot continue a workflow after its profile/page unmounts.
export function useOperationScope() {
  const generation = useRef(0);
  useEffect(() => {
    generation.current += 1;
    return () => { generation.current += 1; };
  }, []);
  return useCallback(() => {
    const started = generation.current;
    return () => generation.current === started;
  }, []);
}
