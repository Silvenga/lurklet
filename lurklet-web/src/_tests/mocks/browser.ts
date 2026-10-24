import { setupWorker } from "msw/browser";
import { handlers } from "./handlers";
import { MOCK_OPTIONS } from "./options";

const worker = setupWorker(...handlers);

await worker.start(MOCK_OPTIONS);

import.meta.hot?.dispose(() => worker.stop());
