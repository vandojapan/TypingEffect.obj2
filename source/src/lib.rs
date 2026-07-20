use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use aviutl2::{AnyResult, module::ScriptModuleFunctions};
use lindera::dictionary::load_dictionary;
use lindera::mode::Mode;
use lindera::segmenter::Segmenter;
use lindera::tokenizer::Tokenizer;
use pinyin::ToPinyin;

const TYPING_EFFECT_VERSION: &str = "0.5.3";
const LINDERA_VERSION: &str = "4.0.0";
const AVIUTL2_RS_VERSION: &str = "0.40.0";
const LANGUAGE_JAPANESE: i32 = 0;
const LANGUAGE_CHINESE: i32 = 1;
const CACHE_LIMIT: usize = 256;

static JAPANESE_TOKENIZER: OnceLock<Result<Tokenizer, String>> = OnceLock::new();
static CHINESE_TOKENIZER: OnceLock<Result<Tokenizer, String>> = OnceLock::new();
static STATE: OnceLock<Mutex<RuntimeState>> = OnceLock::new();

#[derive(Default)]
struct RuntimeState {
    cache: HashMap<String, String>,
    last_error: String,
}

#[derive(Clone, Debug)]
struct TokenInfo {
    surface: String,
    pos: String,
    pos1: String,
    reading: String,
}

fn state() -> &'static Mutex<RuntimeState> {
    STATE.get_or_init(|| Mutex::new(RuntimeState::default()))
}

fn build_tokenizer(dictionary_uri: &str) -> Result<Tokenizer, String> {
    let dictionary = load_dictionary(dictionary_uri).map_err(|e| e.to_string())?;
    let segmenter = Segmenter::new(Mode::Normal, dictionary, None);
    Ok(Tokenizer::new(segmenter))
}

fn japanese_tokenizer() -> Result<&'static Tokenizer, String> {
    JAPANESE_TOKENIZER
        .get_or_init(|| build_tokenizer("embedded://ipadic"))
        .as_ref()
        .map_err(Clone::clone)
}

fn chinese_tokenizer() -> Result<&'static Tokenizer, String> {
    CHINESE_TOKENIZER
        .get_or_init(|| build_tokenizer("embedded://cc-cedict"))
        .as_ref()
        .map_err(Clone::clone)
}

fn set_last_error(message: impl Into<String>) {
    if let Ok(mut runtime) = state().lock() {
        runtime.last_error = message.into();
    }
}

fn clear_last_error() {
    set_last_error(String::new());
}

fn last_error_text() -> String {
    state()
        .lock()
        .map(|runtime| runtime.last_error.clone())
        .unwrap_or_else(|_| "内部状態のロックに失敗しました".to_owned())
}

fn is_katakana(text: &str) -> bool {
    !text.is_empty()
        && text.chars().all(|c| {
            matches!(c as u32, 0x30A1..=0x30F6 | 0x30FD..=0x30FF) || c == 'ー'
        })
}

fn tokenize_japanese(text: &str) -> Result<Vec<TokenInfo>, String> {
    let mut tokens = japanese_tokenizer()?.tokenize(text).map_err(|e| e.to_string())?;
    let mut result = Vec::with_capacity(tokens.len());

    for token in tokens.iter_mut() {
        let surface = token.surface.as_ref().to_owned();
        let details = token.details();
        let pos = details.first().copied().unwrap_or("UNK").to_owned();
        let pos1 = details.get(1).copied().unwrap_or("*").to_owned();
        // Lindera IPADIC 4.0.0 の詳細配列は
        // 0:品詞, 1:品詞細分類1, ... 6:原形, 7:読み, 8:発音。
        let reading = details
            .get(7)
            .copied()
            .filter(|value| *value != "*" && is_katakana(value))
            .unwrap_or(surface.as_str())
            .to_owned();

        result.push(TokenInfo {
            surface,
            pos,
            pos1,
            reading,
        });
    }

    Ok(result)
}


fn is_han_char(c: char) -> bool {
    matches!(
        c as u32,
        0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xF900..=0xFAFF
            | 0x20000..=0x2FA1F
    )
}

fn contains_han(text: &str) -> bool {
    text.chars().any(is_han_char)
}

fn plain_pinyin_char(c: char) -> Option<char> {
    Some(match c {
        'ā' | 'á' | 'ǎ' | 'à' | 'Ā' | 'Á' | 'Ǎ' | 'À' => 'a',
        'ē' | 'é' | 'ě' | 'è' | 'Ē' | 'É' | 'Ě' | 'È' | 'ê' | 'Ê' => 'e',
        'ī' | 'í' | 'ǐ' | 'ì' | 'Ī' | 'Í' | 'Ǐ' | 'Ì' => 'i',
        'ō' | 'ó' | 'ǒ' | 'ò' | 'Ō' | 'Ó' | 'Ǒ' | 'Ò' => 'o',
        'ū' | 'ú' | 'ǔ' | 'ù' | 'Ū' | 'Ú' | 'Ǔ' | 'Ù' => 'u',
        'ü' | 'ǖ' | 'ǘ' | 'ǚ' | 'ǜ' | 'Ü' | 'Ǖ' | 'Ǘ' | 'Ǚ' | 'Ǜ' => 'v',
        'ń' | 'ň' | 'ǹ' | 'Ń' | 'Ň' | 'Ǹ' => 'n',
        'ḿ' | 'Ḿ' => 'm',
        _ if c.is_ascii_alphabetic() => c.to_ascii_lowercase(),
        _ if c == '\'' => '\'',
        _ => return None,
    })
}

fn normalize_pinyin(raw: &str) -> String {
    let normalized = raw.replace("u:", "v").replace("U:", "v");
    let mut result = String::new();

    for syllable in normalized.split_whitespace() {
        let mut plain = String::new();
        for c in syllable.chars() {
            if c.is_ascii_digit() || matches!(c, '-' | '·' | '・') {
                continue;
            }
            if let Some(mapped) = plain_pinyin_char(c) {
                plain.push(mapped);
            }
        }
        if plain.is_empty() {
            continue;
        }

        // a/e/o で始まる音節は、前音節との誤結合を避けるためアポストロフィを入れる。
        if !result.is_empty()
            && plain
                .chars()
                .next()
                .is_some_and(|c| matches!(c, 'a' | 'e' | 'o'))
            && !result.ends_with('\'')
        {
            result.push('\'');
        }
        result.push_str(&plain);
    }

    result
}

fn fallback_pinyin(surface: &str) -> Option<String> {
    let mut result = String::new();
    for ch in surface.chars() {
        if let Some(pinyin) = ch.to_pinyin() {
            result.push_str(&normalize_pinyin(pinyin.plain()));
        } else if is_han_char(ch) {
            return None;
        } else if ch.is_ascii_alphanumeric() {
            result.push(ch.to_ascii_lowercase());
        } else if ch.is_whitespace() {
            result.push(' ');
        } else {
            return None;
        }
    }
    (!result.is_empty()).then_some(result)
}

#[derive(Clone, Debug)]
struct ChineseTokenInfo {
    surface: String,
    pinyin: Option<String>,
}

fn tokenize_chinese(text: &str) -> Result<Vec<ChineseTokenInfo>, String> {
    let mut tokens = chinese_tokenizer()?.tokenize(text).map_err(|e| e.to_string())?;
    let mut result = Vec::with_capacity(tokens.len());

    for token in tokens.iter_mut() {
        let surface = token.surface.as_ref().to_owned();
        let dictionary_pinyin = token
            .get("pinyin")
            .filter(|value| !value.is_empty() && *value != "*")
            .map(normalize_pinyin)
            .filter(|value| !value.is_empty());
        let pinyin = dictionary_pinyin.or_else(|| fallback_pinyin(&surface));
        result.push(ChineseTokenInfo { surface, pinyin });
    }

    Ok(result)
}

fn annotate_chinese(text: &str) -> Result<String, String> {
    let tokens = tokenize_chinese(text)?;
    let mut result = String::new();

    for token in tokens {
        if !result.is_empty() {
            result.push('/');
        }

        if contains_han(&token.surface) {
            if let Some(pinyin) = token.pinyin.as_deref().filter(|value| !value.is_empty()) {
                result.push_str("<i>");
                result.push_str(pinyin);
                result.push_str("</i>");
            }
        }
        result.push_str(&escape_target(&token.surface));
    }

    Ok(result)
}

fn is_function_word(token: &TokenInfo) -> bool {
    matches!(token.pos.as_str(), "助詞" | "助動詞" | "記号" | "補助記号")
        || matches!(token.pos1.as_str(), "接尾" | "接尾辞" | "非自立可能")
}

fn is_prefix_word(token: &TokenInfo) -> bool {
    matches!(token.pos.as_str(), "接頭詞" | "接頭辞")
        || matches!(token.pos1.as_str(), "接頭" | "接頭辞")
}

fn group_bunsetsu(tokens: Vec<TokenInfo>) -> Vec<Vec<TokenInfo>> {
    let mut groups = Vec::new();
    let mut current: Vec<TokenInfo> = Vec::new();
    let mut has_content = false;

    for token in tokens {
        let functional = is_function_word(&token);
        let prefix = is_prefix_word(&token);
        let noun_chain = current
            .last()
            .map(|last| last.pos == "名詞" && token.pos == "名詞")
            .unwrap_or(false);

        if !current.is_empty() && has_content && !functional && !noun_chain && !prefix {
            groups.push(std::mem::take(&mut current));
            has_content = false;
        } else if !current.is_empty() && prefix && has_content {
            groups.push(std::mem::take(&mut current));
            has_content = false;
        }

        if !functional && !prefix {
            has_content = true;
        }
        current.push(token);
    }

    if !current.is_empty() {
        groups.push(current);
    }
    groups
}

fn hiragana_char(c: char) -> char {
    let value = c as u32;
    if (0x30A1..=0x30F6).contains(&value) {
        char::from_u32(value - 0x60).unwrap_or(c)
    } else {
        c
    }
}

fn romaji_for(kana: &str) -> Option<&'static str> {
    Some(match kana {
        "あ" => "a", "い" => "i", "う" => "u", "え" => "e", "お" => "o",
        "か" => "ka", "き" => "ki", "く" => "ku", "け" => "ke", "こ" => "ko",
        "が" => "ga", "ぎ" => "gi", "ぐ" => "gu", "げ" => "ge", "ご" => "go",
        "さ" => "sa", "し" => "shi", "す" => "su", "せ" => "se", "そ" => "so",
        "ざ" => "za", "じ" => "ji", "ず" => "zu", "ぜ" => "ze", "ぞ" => "zo",
        "た" => "ta", "ち" => "chi", "つ" => "tsu", "て" => "te", "と" => "to",
        "だ" => "da", "ぢ" => "di", "づ" => "du", "で" => "de", "ど" => "do",
        "な" => "na", "に" => "ni", "ぬ" => "nu", "ね" => "ne", "の" => "no",
        "は" => "ha", "ひ" => "hi", "ふ" => "fu", "へ" => "he", "ほ" => "ho",
        "ば" => "ba", "び" => "bi", "ぶ" => "bu", "べ" => "be", "ぼ" => "bo",
        "ぱ" => "pa", "ぴ" => "pi", "ぷ" => "pu", "ぺ" => "pe", "ぽ" => "po",
        "ま" => "ma", "み" => "mi", "む" => "mu", "め" => "me", "も" => "mo",
        "や" => "ya", "ゆ" => "yu", "よ" => "yo",
        "ら" => "ra", "り" => "ri", "る" => "ru", "れ" => "re", "ろ" => "ro",
        "わ" => "wa", "ゐ" => "wi", "ゑ" => "we", "を" => "wo", "ゔ" => "vu",
        "きゃ" => "kya", "きゅ" => "kyu", "きょ" => "kyo",
        "ぎゃ" => "gya", "ぎゅ" => "gyu", "ぎょ" => "gyo",
        "しゃ" => "sha", "しゅ" => "shu", "しょ" => "sho",
        "じゃ" => "ja", "じゅ" => "ju", "じょ" => "jo",
        "ちゃ" => "cha", "ちゅ" => "chu", "ちょ" => "cho",
        "にゃ" => "nya", "にゅ" => "nyu", "にょ" => "nyo",
        "ひゃ" => "hya", "ひゅ" => "hyu", "ひょ" => "hyo",
        "びゃ" => "bya", "びゅ" => "byu", "びょ" => "byo",
        "ぴゃ" => "pya", "ぴゅ" => "pyu", "ぴょ" => "pyo",
        "みゃ" => "mya", "みゅ" => "myu", "みょ" => "myo",
        "りゃ" => "rya", "りゅ" => "ryu", "りょ" => "ryo",
        "ふぁ" => "fa", "ふぃ" => "fi", "ふぇ" => "fe", "ふぉ" => "fo",
        "うぃ" => "wi", "うぇ" => "we", "うぉ" => "who",
        "しぇ" => "she", "じぇ" => "je", "ちぇ" => "che",
        "つぁ" => "tsa", "つぃ" => "tsi", "つぇ" => "tse", "つぉ" => "tso",
        "てぃ" => "thi", "てゅ" => "thu", "でぃ" => "dhi", "でゅ" => "dhu",
        "ぁ" => "xa", "ぃ" => "xi", "ぅ" => "xu", "ぇ" => "xe", "ぉ" => "xo",
        "ゃ" => "xya", "ゅ" => "xyu", "ょ" => "xyo", "ゎ" => "xwa",
        "、" => ",", "。" => ".", "！" => "!", "？" => "?", "ー" => "-",
        " " | "　" => " ", "\n" => "\n",
        _ => return None,
    })
}

fn lookup_romaji(chars: &[char], index: usize) -> Option<(&'static str, usize)> {
    if index + 1 < chars.len() {
        let pair = [chars[index], chars[index + 1]].iter().collect::<String>();
        if let Some(value) = romaji_for(&pair) {
            return Some((value, 2));
        }
    }
    let single = chars[index].to_string();
    romaji_for(&single).map(|value| (value, 1))
}

fn next_romaji(chars: &[char], index: usize) -> String {
    lookup_romaji(chars, index)
        .map(|(value, _)| value.to_owned())
        .unwrap_or_default()
}

fn kana_to_romaji(input: &str) -> String {
    let chars: Vec<char> = input.chars().map(hiragana_char).collect();
    let mut output = String::new();
    let mut index = 0;

    while index < chars.len() {
        match chars[index] {
            'っ' => {
                let next = next_romaji(&chars, index + 1);
                let first = next.chars().next();
                if first.is_some_and(|c| !matches!(c, 'a' | 'i' | 'u' | 'e' | 'o' | 'n')) {
                    output.push(first.unwrap());
                } else {
                    output.push_str("xtu");
                }
                index += 1;
            }
            'ん' => {
                let next = next_romaji(&chars, index + 1);
                let first = next.chars().next();
                if first.is_some_and(|c| matches!(c, 'a' | 'i' | 'u' | 'e' | 'o' | 'y' | 'n')) {
                    output.push_str("n'");
                } else {
                    output.push('n');
                }
                index += 1;
            }
            _ => {
                if let Some((value, consumed)) = lookup_romaji(&chars, index) {
                    output.push_str(value);
                    index += consumed;
                } else {
                    output.push(chars[index]);
                    index += 1;
                }
            }
        }
    }

    output
}

fn escape_target(text: &str) -> String {
    let mut result = String::new();
    let mut index = 0;
    while index < text.len() {
        let rest = &text[index..];
        if rest.starts_with("\\/") {
            result.push_str("\\/");
            index += 2;
            continue;
        }
        let ch = rest.chars().next().expect("valid char boundary");
        if ch == '/' {
            result.push_str("\\/");
        } else {
            result.push(ch);
        }
        index += ch.len_utf8();
    }
    result
}

fn split_manual(text: &str) -> Vec<String> {
    let mut sections = Vec::new();
    let mut buffer = String::new();
    let mut in_tag = false;
    let mut index = 0;

    while index < text.len() {
        let rest = &text[index..];
        if rest.starts_with("<i>") {
            buffer.push_str("<i>");
            in_tag = true;
            index += 3;
        } else if in_tag && (rest.starts_with("</i>") || rest.starts_with("<i/>")) {
            buffer.push_str(&rest[..4]);
            in_tag = false;
            index += 4;
        } else if rest.starts_with("\\/") {
            buffer.push_str("\\/");
            index += 2;
        } else {
            let ch = rest.chars().next().expect("valid char boundary");
            if !in_tag && ch == '/' {
                sections.push(std::mem::take(&mut buffer));
            } else {
                buffer.push(ch);
            }
            index += ch.len_utf8();
        }
    }

    sections.push(buffer);
    sections
}

fn normalize_manual(section: &str) -> Option<String> {
    if !section.starts_with("<i>") {
        return None;
    }
    let relative = section[3..]
        .find("</i>")
        .or_else(|| section[3..].find("<i/>"))?;
    let close = relative + 3;
    let input = &section[3..close];
    let target = &section[close + 4..];
    Some(format!("<i>{input}</i>{}", escape_target(target)))
}

fn analyze_section(section: &str, language: i32) -> Result<String, String> {
    if section.is_empty() {
        return Ok(String::new());
    }
    if let Some(manual) = normalize_manual(section) {
        return Ok(manual);
    }

    let plain = section.replace("\\/", "/");
    match language {
        LANGUAGE_JAPANESE => {
            let tokens = tokenize_japanese(&plain)?;
            let mut result = String::new();

            for group in group_bunsetsu(tokens) {
                let mut target = String::new();
                let mut reading = String::new();
                for token in group {
                    target.push_str(&token.surface);
                    reading.push_str(&token.reading);
                }
                if !result.is_empty() {
                    result.push('/');
                }
                result.push_str("<i>");
                result.push_str(&kana_to_romaji(&reading));
                result.push_str("</i>");
                result.push_str(&escape_target(&target));
            }

            Ok(result)
        }
        LANGUAGE_CHINESE => annotate_chinese(&plain),
        _ => Err(format!("未対応の言語番号です: {language}")),
    }
}

fn annotate_text(text: &str, language: i32) -> String {
    let cache_key = format!("{language}\n{text}");
    if let Ok(runtime) = state().lock() {
        if let Some(value) = runtime.cache.get(&cache_key) {
            return value.clone();
        }
    }

    let mut result = String::new();
    let mut error = None;
    for section in split_manual(text) {
        let annotated = match analyze_section(&section, language) {
            Ok(value) => value,
            Err(message) => {
                error = Some(message);
                section
            }
        };
        if !annotated.is_empty() {
            if !result.is_empty() {
                result.push('/');
            }
            result.push_str(&annotated);
        }
    }
    if result.is_empty() {
        result.push_str(text);
    }

    if let Ok(mut runtime) = state().lock() {
        if let Some(message) = error {
            runtime.last_error = message;
        } else {
            runtime.last_error.clear();
        }
        if runtime.cache.len() >= CACHE_LIMIT {
            runtime.cache.clear();
        }
        runtime.cache.insert(cache_key, result.clone());
    }

    result
}

#[aviutl2::plugin(ScriptModule)]
struct TypingEffect;

impl aviutl2::module::ScriptModule for TypingEffect {
    fn new(_info: aviutl2::AviUtl2Info) -> AnyResult<Self> {
        Ok(Self)
    }

    fn plugin_info(&self) -> aviutl2::module::ScriptModuleTable {
        aviutl2::module::ScriptModuleTable {
            information: format!(
                "TypingEffect Full Rust / aviutl2-rs {AVIUTL2_RS_VERSION} / Lindera {LINDERA_VERSION} / v{TYPING_EFFECT_VERSION}"
            ),
            functions: Self::functions(),
        }
    }
}

#[aviutl2::module::functions]
impl TypingEffect {
    fn annotate(&self, input: String, language: i32) -> AnyResult<String> {
        Ok(annotate_text(&input, language))
    }

    fn available(&self) -> AnyResult<bool> {
        match (japanese_tokenizer(), chinese_tokenizer()) {
            (Ok(_), Ok(_)) => {
                clear_last_error();
                Ok(true)
            }
            (Err(message), _) => {
                set_last_error(format!("IPADIC: {message}"));
                Ok(false)
            }
            (_, Err(message)) => {
                set_last_error(format!("CC-CEDICT: {message}"));
                Ok(false)
            }
        }
    }

    fn last_error(&self) -> AnyResult<String> {
        Ok(last_error_text())
    }

    fn diagnose(&self) -> AnyResult<String> {
        clear_last_error();
        let japanese = analyze_section("今日は", LANGUAGE_JAPANESE);
        let chinese = analyze_section("今天天气很好", LANGUAGE_CHINESE);

        match (japanese, chinese) {
            (Ok(ja), Ok(zh)) if !ja.is_empty() && !zh.is_empty() => Ok(format!(
                "OK: 日本語・中文を解析できました\nTypingEffect: {TYPING_EFFECT_VERSION}\naviutl2-rs: {AVIUTL2_RS_VERSION}\nエンジン: Lindera {LINDERA_VERSION}\n辞書: IPADIC / CC-CEDICT（mod2内蔵）\n中文フォールバック: pinyin 0.11.0\n構成: Full Rust / aviutl2クレート / 単一mod2\n日本語: {ja}\n中文: {zh}"
            )),
            (Err(message), _) => {
                let message = format!("日本語解析: {message}");
                set_last_error(&message);
                Ok(format!("NG: {message}"))
            }
            (_, Err(message)) => {
                let message = format!("中文解析: {message}");
                set_last_error(&message);
                Ok(format!("NG: {message}"))
            }
            _ => {
                let message = "解析結果が空です";
                set_last_error(message);
                Ok(format!("NG: {message}"))
            }
        }
    }
}

aviutl2::register_script_module!(TypingEffect);


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_sections_are_preserved() {
        assert_eq!(
            split_manual("今日は/<i>yoi</i>良い/天気です"),
            vec!["今日は", "<i>yoi</i>良い", "天気です"]
        );
    }

    #[test]
    fn slash_escape_is_preserved() {
        assert_eq!(split_manual("A\\/B/C"), vec!["A\\/B", "C"]);
        assert_eq!(escape_target("A/B"), "A\\/B");
    }

    #[test]
    fn kana_conversion_matches_ime_style() {
        assert_eq!(kana_to_romaji("キョウハ"), "kyouha");
        assert_eq!(kana_to_romaji("キッテ"), "kitte");
        assert_eq!(kana_to_romaji("シンアイ"), "shin'ai");
    }

    #[test]
    fn manual_bopomofo_is_accepted() {
        assert_eq!(
            normalize_manual("<i>ㄋㄧˇ ㄏㄠˇ</i>你好"),
            Some("<i>ㄋㄧˇ ㄏㄠˇ</i>你好".to_owned())
        );
    }


    #[test]
    fn pinyin_is_normalized_for_ime_input() {
        assert_eq!(normalize_pinyin("Jin1 tian1"), "jintian");
        assert_eq!(normalize_pinyin("Xi1 an1"), "xi'an");
        assert_eq!(normalize_pinyin("nü3 hai2"), "nvhai");
        assert_eq!(normalize_pinyin("LÜ4"), "lv");
    }

    #[test]
    fn character_fallback_generates_plain_pinyin() {
        assert_eq!(fallback_pinyin("中国"), Some("zhongguo".to_owned()));
    }

    #[test]
    fn chinese_dictionary_annotation_is_generated() {
        let annotated = analyze_section("今天天气很好", LANGUAGE_CHINESE).unwrap();
        assert!(annotated.contains("<i>"));
        assert!(annotated.contains("今天"));
        assert!(annotated.contains("天气"));
    }
}
