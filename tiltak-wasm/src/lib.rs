use board_game_traits::{Color, Position as PositionTrait};
use pgn_traits::PgnPosition;
use std::env;
use std::error::Error;
use std::fmt::Display;
use std::str::FromStr;
use std::time::{Duration, Instant};
use tiltak::position::{Komi, Position};
use tokio::sync::mpsc::error::TryRecvError;
use wasm_bindgen_futures::js_sys::{self, Array};

use crate::search::MctsSetting;
use std::any::Any;
use tiltak::search::{self, MonteCarloTree};
use tokio::sync::mpsc;

use wasm_bindgen::prelude::*;

#[derive(Debug)]
pub enum TeiError {
    NoInput,
    NoOutput(String),
    InvalidInput(String),
}

impl Display for TeiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TeiError::NoInput => write!(f, "input channel closed"),
            TeiError::NoOutput(s) => write!(f, "output channel closed when writing \"{}\"", s),
            TeiError::InvalidInput(s) => write!(f, "invalid tei input \"{}\"", s),
        }
    }
}

impl Error for TeiError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }

    fn description(&self) -> &str {
        "description() is deprecated; use Display"
    }

    fn cause(&self) -> Option<&dyn Error> {
        self.source()
    }
}

impl From<mpsc::error::SendError<String>> for TeiError {
    fn from(error: mpsc::error::SendError<String>) -> Self {
        TeiError::NoOutput(error.0)
    }
}

pub fn start_engine(
    output_callback: js_sys::Function,
) -> (js_sys::Promise, mpsc::UnboundedSender<String>) {
    let (input_sender, input_recv) = mpsc::unbounded_channel();
    let (output_sender, mut output_recv) = mpsc::unbounded_channel();
    let send_output_promise = wasm_bindgen_futures::future_to_promise(async move {
        loop {
            let val: String = output_recv.recv().await.unwrap();
            let args = js_sys::Array::new();
            args.push(&val.into());
            output_callback.apply(&JsValue::NULL, &args).unwrap();
        }
    });
    let tei_promise =
        wasm_bindgen_futures::future_to_promise(tei_jsvalue(input_recv, output_sender));
    let promises = js_sys::Array::new();
    promises.push(&send_output_promise);
    promises.push(&tei_promise);
    (js_sys::Promise::all(&promises), input_sender)
}

pub async fn tei_jsvalue(
    input: mpsc::UnboundedReceiver<String>,
    output: mpsc::UnboundedSender<String>,
) -> Result<JsValue, JsValue> {
    tei(input, output)
        .await
        .map(|_| JsValue::UNDEFINED)
        .map_err(|err| JsValue::from_str(&err.to_string()))
}

pub async fn tei(
    mut input: mpsc::UnboundedReceiver<String>,
    output: mpsc::UnboundedSender<String>,
) -> Result<(), TeiError> {
    let is_slatebot = env::args().any(|arg| arg == "--slatebot");

    while input.recv().await.unwrap() != "tei" {}

    output.send("id name Tiltak".to_string())?;
    output.send("id author Morten Lohne".to_string())?;
    output.send("option name HalfKomi type spin default 0 min -10 max 10".to_string())?;
    output.send("teiok".to_string())?;

    // Position stored in a `dyn Any` variable, because it can be any size
    let mut position: Option<Box<dyn Any>> = None;
    let mut size: Option<usize> = None;
    let mut komi = Komi::default();

    loop {
        let line = input.recv().await.unwrap();
        let mut words = line.split_whitespace();
        match words.next().unwrap() {
            "quit" => break Ok(()),
            "isready" => output.send("readyok".to_string())?,
            "setoption" => {
                if [
                    words.next().unwrap_or_default(),
                    words.next().unwrap_or_default(),
                    words.next().unwrap_or_default(),
                ]
                .join(" ")
                    == "name HalfKomi value"
                {
                    if let Some(k) = words
                        .next()
                        .and_then(|komi_string| komi_string.parse::<i8>().ok())
                        .and_then(Komi::from_half_komi)
                    {
                        komi = k;
                    } else {
                        return Err(TeiError::InvalidInput(format!(
                            "Invalid komi setting \"{}\"",
                            line
                        )));
                    }
                } else {
                    return Err(TeiError::InvalidInput(format!(
                        "Invalid setoption string \"{}\"",
                        line
                    )));
                }
            }
            "teinewgame" => {
                let size_string = words.next();
                size = size_string.and_then(|s| usize::from_str(s).ok());
                position = None;

                match size {
                    Some(4) | Some(5) | Some(6) => (),
                    _ => {
                        return Err(TeiError::InvalidInput(format!(
                            "Error: Unsupported size {}",
                            size.unwrap_or_default()
                        )))
                    }
                }
            }
            "position" => {
                position = match size {
                    None => {
                        return Err(TeiError::InvalidInput(
                            "Received position without receiving teinewgame string".to_string(),
                        ))
                    }
                    Some(4) => Some(Box::new(parse_position_string::<4>(&line, komi)?)),
                    Some(5) => Some(Box::new(parse_position_string::<5>(&line, komi)?)),
                    Some(6) => Some(Box::new(parse_position_string::<6>(&line, komi)?)),
                    Some(s) => {
                        return Err(TeiError::InvalidInput(format!("Unsupported size {}", s)))
                    }
                }
            }
            "go" => match size {
                Some(4) => {
                    parse_go_string::<4>(
                        &mut input,
                        &output,
                        &line,
                        position.as_ref().and_then(|p| p.downcast_ref()).unwrap(),
                        is_slatebot,
                    )
                    .await?
                }
                Some(5) => {
                    parse_go_string::<5>(
                        &mut input,
                        &output,
                        &line,
                        position.as_ref().and_then(|p| p.downcast_ref()).unwrap(),
                        is_slatebot,
                    )
                    .await?
                }
                Some(6) => {
                    parse_go_string::<6>(
                        &mut input,
                        &output,
                        &line,
                        position.as_ref().and_then(|p| p.downcast_ref()).unwrap(),
                        is_slatebot,
                    )
                    .await?
                }
                Some(s) => {
                    return Err(TeiError::InvalidInput(format!(
                        "Error: Unsupported size {}",
                        s
                    )))
                }
                None => {
                    return Err(TeiError::InvalidInput(
                        "Error: Received go without receiving teinewgame string".to_string(),
                    ))
                }
            },
            s => return Err(TeiError::InvalidInput(format!("Unknown command \"{}\"", s))),
        }
    }
}

fn parse_position_string<const S: usize>(line: &str, komi: Komi) -> Result<Position<S>, TeiError> {
    let mut words_iter = line.split_whitespace();
    words_iter.next(); // position
    let mut position = match words_iter.next() {
        Some("startpos") => Position::start_position_with_komi(komi),
        Some("tps") => {
            let tps: String = (&mut words_iter).take(3).collect::<Vec<_>>().join(" ");
            <Position<S>>::from_fen_with_komi(&tps, komi).unwrap()
        }
        _ => {
            return Err(TeiError::InvalidInput(
                "Expected \"startpos\" or \"tps\" to specify position.".to_string(),
            ))
        }
    };

    match words_iter.next() {
        Some("moves") => {
            for move_string in words_iter {
                position.do_move(position.move_from_san(move_string).unwrap());
            }
        }
        Some(s) => {
            return Err(TeiError::InvalidInput(format!(
                "Expected \"moves\" in \"{}\", got \"{}\".",
                line, s
            )))
        }
        None => (),
    }
    Ok(position)
}

async fn parse_go_string<const S: usize>(
    input: &mut mpsc::UnboundedReceiver<String>,
    output: &mpsc::UnboundedSender<String>,
    line: &str,
    position: &Position<S>,
    is_slatebot: bool,
) -> Result<(), TeiError> {
    let mut words = line.split_whitespace();
    words.next(); // go

    let mcts_settings = if is_slatebot {
        MctsSetting::default().add_rollout_depth(200)
    } else {
        MctsSetting::default()
    };

    match words.next() {
        Some("movetime") => {
            let msecs = words.next().unwrap();
            let movetime = Duration::from_millis(u64::from_str(msecs).unwrap());
            let start_time = Instant::now();
            let mut tree = search::MonteCarloTree::with_settings(position.clone(), mcts_settings);

            for i in 0.. {
                let nodes_to_search = (200.0 * f64::powf(1.26, i as f64)) as u64;
                let mut oom = false;
                for _ in 0..nodes_to_search {
                    match input.try_recv() {
                        Ok(line) => match line.as_str() {
                            "stop" => break,
                            "quit" => break,
                            "isready" => output.send("readyok".to_string())?,
                            _ => return Err(TeiError::InvalidInput(line)),
                        },
                        Err(TryRecvError::Empty) => (),
                        Err(TryRecvError::Disconnected) => return Err(TeiError::NoInput),
                    }
                    if tree.select().is_none() {
                        eprintln!("Warning: Search stopped early due to OOM");
                        oom = true;
                        break;
                    };
                }
                let (best_move, best_score) = tree.best_move();
                let pv: Vec<_> = tree.pv().collect();
                output.send(format!(
                    "info depth {} seldepth {} nodes {} score cp {} time {} nps {:.0} pv {}",
                    ((tree.visits() as f64 / 10.0).log2()) as u64,
                    pv.len(),
                    tree.visits(),
                    (best_score * 200.0 - 100.0) as i64,
                    start_time.elapsed().as_millis(),
                    tree.visits() as f32 / start_time.elapsed().as_secs_f32(),
                    pv.iter()
                        .map(|mv| position.move_to_san(mv))
                        .collect::<Vec<String>>()
                        .join(" ")
                ))?;
                if oom || start_time.elapsed().as_secs_f64() > movetime.as_secs_f64() * 0.7 {
                    output.send(format!("bestmove {}", position.move_to_san(&best_move)))?;
                    break;
                }
            }
            Ok(())
        }
        Some("wtime") | Some("btime") | Some("winc") | Some("binc") => {
            let parse_time = |s: Option<&str>| {
                Duration::from_millis(
                    s.and_then(|w| w.parse().ok())
                        .unwrap_or_else(|| panic!("Incorrect go command {}", line)),
                )
            };
            let mut words = line.split_whitespace().skip(1).peekable();
            let mut white_time = Duration::default();
            let mut white_inc = Duration::default();
            let mut black_time = Duration::default();
            let mut black_inc = Duration::default();

            while let Some(word) = words.next() {
                match word {
                    "wtime" => white_time = parse_time(words.next()),
                    "winc" => white_inc = parse_time(words.next()),
                    "btime" => black_time = parse_time(words.next()),
                    "binc" => black_inc = parse_time(words.next()),
                    _ => (),
                }
            }

            let max_time = match position.side_to_move() {
                Color::White => white_time / 5 + white_inc / 2,
                Color::Black => black_time / 5 + black_inc / 2,
            };

            let start_time = Instant::now();

            let mut tree = MonteCarloTree::with_settings(position.clone(), mcts_settings);
            tree.search_for_time(max_time, |tree| {
                let best_score = tree.best_move().1;
                let pv: Vec<_> = tree.pv().collect();
                output
                    .send(format!(
                        "info depth {} seldepth {} nodes {} score cp {} time {} nps {:.0} pv {}",
                        ((tree.visits() as f64 / 10.0).log2()) as u64,
                        pv.len(),
                        tree.visits(),
                        (best_score * 200.0 - 100.0) as i64,
                        start_time.elapsed().as_millis(),
                        tree.visits() as f32 / start_time.elapsed().as_secs_f32(),
                        pv.iter()
                            .map(|mv| position.move_to_san(mv))
                            .collect::<Vec<String>>()
                            .join(" ")
                    ))
                    .unwrap();
            });
            let best_move = tree.best_move().0;

            output.send(format!("bestmove {}", position.move_to_san(&best_move)))?;
            Ok(())
        }
        Some(_) | None => {
            panic!("Invalid go command \"{}\"", line);
        }
    }
}
