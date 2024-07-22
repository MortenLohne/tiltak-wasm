/* eslint-disable no-restricted-globals */

import init, { Data } from 'tiltak-wasm';

init().then(() => {
    const startTime = performance.now()
    const data = new Data();
    const endTime = performance.now()
    console.log(`Loaded pokemon data in ${endTime - startTime} milliseconds`)

    const defaultResult = data.compute("", "", []);
    postMessage(JSON.stringify(defaultResult))

    self.onmessage = function(e) {
        const payload = JSON.parse(e.data)
        // console.log(`Computing with "${payload.type}" and "${payload.phrase}", from "${payload}`)
        const probabilities = data.compute(payload.type, payload.phrase, []);
        postMessage(JSON.stringify(
            probabilities))
    };
});