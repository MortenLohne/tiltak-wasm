/* eslint-disable no-restricted-globals */

import init, { start_engine } from 'tiltak-wasm';

init().then(() => {
    const startTime = performance.now()
    let callback = start_engine(output => {
        console.log(`Received ${output} from engine`);
        postMessage(output);
    });

    const endTime = performance.now()
    console.log(`Started engine in ${endTime - startTime} milliseconds`)

    self.onmessage = e => {
        const payload = e.data;
        console.log(`Sending "${payload}" to engine`);
        callback(payload);
    };
});
