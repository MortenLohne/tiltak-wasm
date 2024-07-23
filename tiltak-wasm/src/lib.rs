use board_game_traits::Position as PositionTrait;
use futures::try_join;
use pgn_traits::PgnPosition;
use std::env;
use std::error::Error;
use std::fmt::Display;
use std::str::FromStr;
use tiltak::position::{Komi, Position};
use tokio::sync::mpsc::error::TryRecvError;
use wasm_bindgen_futures::js_sys;

use crate::search::MctsSetting;
use std::any::Any;
use tiltak::search;
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

#[wasm_bindgen]
pub fn start_engine(output_callback: js_sys::Function) -> JsValue {
    console_error_panic_hook::set_once();
    let (input_sender, input_recv) = mpsc::unbounded_channel();
    let (output_sender, mut output_recv) = mpsc::unbounded_channel();
    let future = async {
        let send_output = async move {
            loop {
                let val: String = output_recv.recv().await.unwrap();
                let args = js_sys::Array::new();
                args.push(&val.into());
                output_callback.apply(&JsValue::NULL, &args).unwrap();
            }
            // Unreachable return statement required to assign a type to the async block
            #[allow(unreachable_code)]
            Ok::<(), JsValue>(())
        };
        let tei_future = tei_jsvalue(input_recv, output_sender);
        try_join!(send_output, tei_future).map(|_| JsValue::undefined())
    };
    let receive_input: Closure<dyn Fn(String)> =
        Closure::new(move |input| input_sender.send(input).unwrap());
    let js_closure = receive_input.as_ref().clone();
    receive_input.forget();
    // Pass the future to the JS runtime
    let _ = wasm_bindgen_futures::future_to_promise(future);
    js_closure
}

pub async fn tei_jsvalue(
    input: mpsc::UnboundedReceiver<String>,
    output: mpsc::UnboundedSender<String>,
) -> Result<(), JsValue> {
    tei(input, output)
        .await
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
        output.send(format!("Received line \"{}\"", line))?;
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
            "stop" => (),
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
    }
    .max_arena_size();

    match words.next() {
        Some("movetime") => {
            let msecs = words.next().unwrap();
            let movetime = u64::from_str(msecs).unwrap();
            let start_time = js_sys::Date::now();
            let mut tree = search::MonteCarloTree::with_settings(position.clone(), mcts_settings);

            for i in 0.. {
                let nodes_to_search = (1000.0 * f64::powf(1.1, i as f64)) as u64;
                let mut exit = false;
                for _ in 0..nodes_to_search {
                    if tree.visits() % 10000 == 0 {
                        yield_().await;
                        match input.try_recv() {
                            Ok(line) => match line.as_str() {
                                "stop" => {
                                    exit = true;
                                    break;
                                }
                                "quit" => return Ok(()),
                                "isready" => output.send("readyok".to_string())?,
                                _ => return Err(TeiError::InvalidInput(line)),
                            },
                            Err(TryRecvError::Empty) => (),
                            Err(TryRecvError::Disconnected) => return Err(TeiError::NoInput),
                        }
                    }
                    if tree.select().is_none() {
                        eprintln!("Warning: Search stopped early due to OOM");
                        exit = true;
                        break;
                    };
                }
                let (best_move, best_score) = tree.best_move();
                let pv: Vec<_> = tree.pv().collect();
                let elapsed = js_sys::Date::now() - start_time;
                output.send(format!(
                    "info depth {} seldepth {} nodes {} score cp {} time {} nps {:.0} pv {}",
                    ((tree.visits() as f64 / 100.0).log2()) as u64,
                    pv.len(),
                    tree.visits(),
                    (best_score * 200.0 - 100.0) as i64,
                    elapsed as u64,
                    tree.visits() as f32 * 1000.0 / elapsed as f32,
                    pv.iter()
                        .map(|mv| position.move_to_san(mv))
                        .collect::<Vec<String>>()
                        .join(" ")
                ))?;
                if exit || elapsed > movetime as f64 * 0.7 {
                    output.send(format!("bestmove {}", position.move_to_san(&best_move)))?;
                    break;
                }
            }
            Ok(())
        }
        Some(_) | None => {
            panic!("Invalid go command \"{}\"", line);
        }
    }
}

/// Yield to other tasks
async fn yield_() {
    worker_timer(1).await.unwrap();
}

/// worker timer, which setTimeout is created by WorkerGlobalScope
/// This is necessary because worker has no access to windows.
pub async fn worker_timer(ms: i32) -> Result<(), JsValue> {
    let promise = js_sys::Promise::new(&mut |yes, _| {
        let global = js_sys::global();
        let scope = global.dyn_into::<web_sys::WorkerGlobalScope>().unwrap();
        scope
            .set_timeout_with_callback_and_timeout_and_arguments_0(&yes, ms)
            .unwrap();
    });
    let js_fut = wasm_bindgen_futures::JsFuture::from(promise);
    js_fut.await?;
    Ok(())
}
