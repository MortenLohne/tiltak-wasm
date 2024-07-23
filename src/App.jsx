import React, {useState} from 'react';
import './App.css';
import Worker from './calc.worker.js';

function App() {
  const [result, setResult] = useState("<null>");
  const [isCalculating, setIsCalculating] = useState(false);
  const [pv, setPv] = useState([]);
  const [nps, setNps] = useState("");
  const [score, setScore] = useState(0);
  const [inputValue, setInputValue] = useState("x6/x6/x6/x6/x6/x6 1 1");
  const [worker] = useState(() => {
    const worker = new Worker();
    worker.onmessage = (e) => {
      setResult(e.data);
      if (e.data.startsWith("bestmove")) {
        setIsCalculating(false);
      }
      else if (e.data.startsWith("info")) {
        setIsCalculating(true);
        const keywords = ["time", "pv", "nps", "depth", "seldepth", "score", "nodes"];
        let time = "";
        let pv = [];
        let nps = "";
        let score = "";
        let currentKeyword = "";
        for (const word of e.data.split(" ")) {
          if (keywords.includes(word)) {
            currentKeyword = word;
          } else {
            if (currentKeyword === "time") {
              time = word;
            } else if (currentKeyword === "pv") {
              pv.push(word);
            } else if (currentKeyword === "nps") {
              nps = word;
            } else if (currentKeyword === "score") {
              score = Number(word);
            }
          }
        }
        setPv(pv);
        setNps(nps);
        setScore(score);
      }
    };
    return worker;
  });

  const initialize = () => {
    worker.postMessage("tei");
    worker.postMessage("teinewgame 6");
    worker.postMessage("setoption name HalfKomi value 4");
  }

  const [initialized, setInitialized] = useState(false);

  const handleInputChange = (event) => {
    setInputValue(event.target.value);
  };

  // const handleKeyDown = (event) => {
  //   if (event.key === 'Enter') {
  //     if (!initialized) {
  //       initialize();
  //       setInitialized(true)
  //     }
  //     let message = `position tps ${inputValue.trim()}`;
  //     console.log(`Sending ${message}`);
  //     worker.postMessage(message);
  //     setInputValue(''); // Optionally clear the input field after pressing Enter
  //   }
  // };

  const handleGo = () => {
    if (!initialized) {
      initialize();
      setInitialized(true);
    }
    if (isCalculating) {
      worker.postMessage("stop");
    }
    let message = `position tps ${inputValue.trim()}`;
    console.log(`Sending ${message}`);
    worker.postMessage(message);
    worker.postMessage("go movetime 1000000");
  }

  const goNextMove = () => {
    const nextMove = pv[0] || "";
    let newInput = inputValue;
    if (inputValue.includes("moves")) {
      newInput += " " + nextMove;
    } else {
      newInput += " moves " + nextMove;
    }
    setInputValue(newInput);

    if (!initialized) {
      initialize();
      setInitialized(true);
    }
    if (isCalculating) {
      worker.postMessage("stop");
    }
    let message = `position tps ${newInput}`;
    console.log(`Sending ${message}`);
    worker.postMessage(message);
    worker.postMessage("go movetime 1000000");
  }

  const resetTps = () => {
    const index = inputValue.indexOf(" moves");
    if (index === - 1) {
      return
    }
    const newInput = inputValue.slice(0, index);
    setInputValue(newInput);

    if (!initialized) {
      initialize();
      setInitialized(true);
    }
    if (isCalculating) {
      worker.postMessage("stop");
    }
    let message = `position tps ${newInput}`;
    console.log(`Sending ${message}`);
    worker.postMessage(message);
    worker.postMessage("go movetime 1000000");
  }

  const undoMove = () => {
    const index = inputValue.lastIndexOf(" ");
    let newInput = inputValue.slice(0, index);
    if (newInput.trim().endsWith(" moves")) {
      newInput = newInput.slice(0, newInput.indexOf(" moves"))
    }
    setInputValue(newInput);

    if (!initialized) {
      initialize();
      setInitialized(true);
    }
    if (isCalculating) {
      worker.postMessage("stop");
    }
    let message = `position tps ${newInput}`;
    console.log(`Sending ${message}`);
    worker.postMessage(message);
    worker.postMessage("go movetime 1000000");
  }

  return (
    <div className="App">
      <header className="App-header">
        <input type="text"
          value={inputValue}
          onChange={handleInputChange}></input>
        <div>
        <button onClick={() => handleGo()}>Go</button>
        <button onClick={() => worker.postMessage("stop")}>Stop</button>
        </div>
        {pv[0] && <button onClick={() => goNextMove()}>{`Continue with ${pv[0]}`}</button>}
        {inputValue.includes("moves") && <button onClick={() => undoMove()}>{`Undo ${inputValue.slice(inputValue.lastIndexOf(" "))}`}</button>}
        {inputValue.includes("moves") && <button onClick={() => resetTps()}>{`Reset to base TPS`}</button>}
        {isCalculating ? <p>Calculating...</p> : <p>Paused</p>}
        {/* <p>{result}</p> */}
        <p>Evaluation: {((score || 0) + 100) / 2}%</p>
        <p>Main line: {pv.join(" ")}</p>
        <p>{nps || 0} nps</p>
      </header>
    </div>
  );
}

export default App;
