import { RealtimeClient, type RealtimeState } from "./realtime";

export type RealtimeStore = {
  state: RealtimeState;
  setState: (s: RealtimeState) => void;
};

export function bindRealtimeState(client: RealtimeClient, store: RealtimeStore) {
  const wrapped = new RealtimeClient((client as any).wsUrl, (client as any).bearerToken, {
    onEvent: (client as any).callbacks?.onEvent,
    onStateChange: (s) => store.setState(s)
  });
  return wrapped;
}
