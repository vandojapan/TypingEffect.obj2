use std::cell::RefCell;
use std::ffi::{c_char, CStr, CString};
use std::ptr;
use std::sync::OnceLock;

use lindera::dictionary::load_dictionary;
use lindera::mode::Mode;
use lindera::segmenter::Segmenter;
use lindera::tokenizer::Tokenizer;

static TOKENIZER: OnceLock<Result<Tokenizer, String>> = OnceLock::new();

thread_local! {
    static LAST_ERROR: RefCell<CString> = RefCell::new(CString::new("").expect("empty CString"));
}

fn set_error(message: impl Into<String>) {
    let clean = message.into().replace('\0', " ");
    LAST_ERROR.with(|slot| {
        *slot.borrow_mut() = CString::new(clean).unwrap_or_else(|_| CString::new("unknown error").unwrap());
    });
}

fn tokenizer() -> Result<&'static Tokenizer, String> {
    TOKENIZER
        .get_or_init(|| {
            let dictionary = load_dictionary("embedded://ipadic").map_err(|e| e.to_string())?;
            let segmenter = Segmenter::new(Mode::Normal, dictionary, None);
            Ok(Tokenizer::new(segmenter))
        })
        .as_ref()
        .map_err(Clone::clone)
}

fn sanitize_field(value: &str) -> String {
    value.replace(['\t', '\r', '\n', ','], " ")
}

fn tokenize_to_mecab_tsv(text: &str) -> Result<String, String> {
    let mut tokens = tokenizer()?.tokenize(text).map_err(|e| e.to_string())?;
    let mut output = String::new();
    for token in tokens.iter_mut() {
        let surface = sanitize_field(token.surface.as_ref());
        let details = token.details();
        let pos = details.first().copied().unwrap_or("UNK");
        let pos1 = details.get(1).copied().unwrap_or("*");
        let base = details.get(6).copied().unwrap_or(token.surface.as_ref());
        let reading = details.get(7).copied().unwrap_or(token.surface.as_ref());
        output.push_str(&surface);
        output.push('\t');
        output.push_str(&format!(
            "{},{},*,*,*,*,{},{},{}\n",
            sanitize_field(pos),
            sanitize_field(pos1),
            sanitize_field(base),
            sanitize_field(reading),
            sanitize_field(reading)
        ));
    }
    output.push_str("EOS\n");
    Ok(output)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn typing_effect_tokenize(
    input: *const c_char,
    output: *mut c_char,
    capacity: usize,
) -> usize {
    if input.is_null() {
        set_error("input is null");
        return 0;
    }
    let input = match unsafe { CStr::from_ptr(input) }.to_str() {
        Ok(value) => value,
        Err(error) => {
            set_error(format!("input is not UTF-8: {error}"));
            return 0;
        }
    };
    let result = match tokenize_to_mecab_tsv(input) {
        Ok(value) => value,
        Err(error) => {
            set_error(error);
            return 0;
        }
    };
    let required = result.len() + 1;
    if output.is_null() || capacity < required {
        return required;
    }
    unsafe {
        ptr::copy_nonoverlapping(result.as_ptr(), output.cast::<u8>(), result.len());
        *output.add(result.len()) = 0;
    }
    required
}

#[unsafe(no_mangle)]
pub extern "C" fn typing_effect_last_error() -> *const c_char {
    LAST_ERROR.with(|slot| slot.borrow().as_ptr())
}
