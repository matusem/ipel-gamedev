# JS SDK Basic Example

```ts
import { SdkApiClient, RealtimeClient } from "../../../js/src";

const api = new SdkApiClient("http://localhost:8080/graphql");
const token = await api.createPublishToken("user-id");

const realtime = new RealtimeClient("ws://localhost:8080/graphql", token.token, {
  onStateChange: (s) => console.log("state:", s),
  onEvent: (e) => console.log("event:", e)
});
realtime.connect();
```
