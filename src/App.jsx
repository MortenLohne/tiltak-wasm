import React, {useState} from 'react';
import './App.css';
import Worker from './calc.worker.js';

function App() {
  const [result, setResult] = useState("<null>");
  const [pv, setPv] = useState([]);
  const [nps, setNps] = useState("");
  const [score, setScore] = useState(0);
  const [inputValue, setInputValue] = useState("x6/x6/x6/x6/x6/x6 1 1");
  const [worker] = useState(() => {
    const worker = new Worker();
    worker.onmessage = (e) => {
      setResult(e.data);
      if (e.data.startsWith("info")) {
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
    let message = `position tps ${inputValue.trim()}`;
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
        {/* <p>{result}</p> */}
        <p>{((score || 0) + 100) / 2}%</p>
        <p>{pv.join(" ")}</p>
        <p>{nps || 0} nps</p>
      </header>
    </div>
  );
}

export default App;
