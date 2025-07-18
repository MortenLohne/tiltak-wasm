use std::error::Error;
use std::fmt::Display;
use std::pin::Pin;
use std::sync::Mutex;
use wasm_bindgen_futures::js_sys;

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

impl From<async_channel::SendError<String>> for TeiError {
    fn from(error: async_channel::SendError<String>) -> Self {
        TeiError::NoOutput(error.0)
    }
}

static TIME_SINCE_YIELD: Mutex<Option<web_time::Instant>> = Mutex::new(None);

struct WebPlatform {}

impl tiltak::tei::Platform for WebPlatform {
    type Instant = web_time::Instant;
    fn yield_fn() -> impl std::future::Future {
        // Only yield to the JS event loop every 100ms at most, since this is a very expensive operation
        let mut time_since_yield = TIME_SINCE_YIELD.lock().unwrap();
        if time_since_yield
            .is_none_or(|time| Self::elapsed_time(&time) > web_time::Duration::from_millis(100))
        {
            *time_since_yield = Some(Self::current_time());
            Box::pin(yield_()) as Pin<Box<dyn std::future::Future<Output = ()>>>
        } else {
            Box::pin(std::future::ready(())) as Pin<Box<dyn std::future::Future<Output = ()>>>
        }
    }

    fn current_time() -> Self::Instant {
        web_time::Instant::now()
    }

    fn elapsed_time(start: &Self::Instant) -> web_time::Duration {
        start.elapsed()
    }
}

/// Start the engine, which will run asynschronously in the background until it crashes
///
/// @param {function(string): void} output_callback - Callback that receives tei output line by line
/// @return {function(string): void} - Send one line of tei input to the engine
#[wasm_bindgen(skip_jsdoc)]
pub fn start_engine(output_callback: js_sys::Function) -> JsValue {
    console_error_panic_hook::set_once();
    let (input_sender, input_recv) = async_channel::unbounded();

    let rust_output_callback = Box::leak(Box::new(move |message: &str| {
        let args = js_sys::Array::new();
        args.push(&message.into());
        if let Err(err) = output_callback.apply(&JsValue::NULL, &args) {
            web_sys::console::error_2(
                &"Tiltak: caught exception from Javascript callback: ".into(),
                &err,
            )
        }
    }));

    let receive_input: Closure<dyn Fn(String)> =
        Closure::new(move |input| input_sender.try_send(input).unwrap());
    let js_closure = receive_input.as_ref().clone();
    receive_input.forget();
    // Pass the future to the JS runtime
    let _ = wasm_bindgen_futures::future_to_promise(tei_jsvalue(input_recv, rust_output_callback));
    js_closure
}

pub async fn tei_jsvalue<F>(
    input: async_channel::Receiver<String>,
    output: &F,
) -> Result<JsValue, JsValue>
where
    F: Fn(&str),
{
    let result = tiltak::tei::tei::<_, WebPlatform>(false, false, input, output).await;
    // .map(|()| JsValue::UNDEFINED)
    // .map_err(|err| JsValue::from_str(&err.to_string()))
    Ok(JsValue::undefined())
}

/// Yield to other tasks
async fn yield_() {
    worker_timer(0).await.unwrap();
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
