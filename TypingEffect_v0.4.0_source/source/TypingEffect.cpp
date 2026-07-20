#include <windows.h>

#include <algorithm>
#include <cctype>
#include <mutex>
#include <string>
#include <unordered_map>
#include <utility>
#include <vector>

#include "sdk/module2.h"

namespace {

HMODULE g_self = nullptr;
HMODULE g_lindera_dll = nullptr;
std::mutex g_mutex;
std::unordered_map<std::string, std::string> g_cache;
std::string g_last_error;
std::wstring g_lindera_path;

using tokenize_fn = size_t (__cdecl*)(const char*, char*, size_t);
using last_error_fn = const char* (__cdecl*)();

tokenize_fn p_tokenize = nullptr;
last_error_fn p_last_error = nullptr;

std::wstring module_dir() {
    wchar_t path[MAX_PATH] = {};
    GetModuleFileNameW(g_self, path, MAX_PATH);
    std::wstring value(path);
    const auto slash = value.find_last_of(L"\\/");
    return slash == std::wstring::npos ? L"." : value.substr(0, slash);
}

std::wstring utf8_to_wide(const std::string& text) {
    if (text.empty()) return {};
    const int size = MultiByteToWideChar(CP_UTF8, 0, text.data(), static_cast<int>(text.size()), nullptr, 0);
    if (size <= 0) return {};
    std::wstring out(size, L'\0');
    MultiByteToWideChar(CP_UTF8, 0, text.data(), static_cast<int>(text.size()), out.data(), size);
    return out;
}

std::string wide_to_utf8(const std::wstring& text) {
    if (text.empty()) return {};
    const int size = WideCharToMultiByte(CP_UTF8, 0, text.data(), static_cast<int>(text.size()), nullptr, 0, nullptr, nullptr);
    if (size <= 0) return {};
    std::string out(size, '\0');
    WideCharToMultiByte(CP_UTF8, 0, text.data(), static_cast<int>(text.size()), out.data(), size, nullptr, nullptr);
    return out;
}

bool load_lindera() {
    if (g_lindera_dll) return true;
    const auto dir = module_dir();
    const std::vector<std::wstring> candidates = {
        dir + L"\\TypingEffectLindera.dll",
        dir + L"\\lib\\TypingEffectLindera.dll",
        L"TypingEffectLindera.dll"
    };
    for (const auto& path : candidates) {
        g_lindera_dll = LoadLibraryW(path.c_str());
        if (g_lindera_dll) { g_lindera_path = path; break; }
    }
    if (!g_lindera_dll) {
        g_last_error = "TypingEffectLindera.dllが見つかりません";
        return false;
    }
    p_tokenize = reinterpret_cast<tokenize_fn>(GetProcAddress(g_lindera_dll, "typing_effect_tokenize"));
    p_last_error = reinterpret_cast<last_error_fn>(GetProcAddress(g_lindera_dll, "typing_effect_last_error"));
    if (!p_tokenize) {
        g_last_error = "LinderaブリッジAPIを読み込めません";
        FreeLibrary(g_lindera_dll);
        g_lindera_dll = nullptr;
        return false;
    }
    return true;
}

std::string lindera_tokenize(const std::string& text) {
    if (!load_lindera()) return {};
    const size_t required = p_tokenize(text.c_str(), nullptr, 0);
    if (required == 0) {
        g_last_error = p_last_error ? p_last_error() : "Linderaの解析に失敗しました";
        return {};
    }
    std::vector<char> buffer(required);
    if (p_tokenize(text.c_str(), buffer.data(), buffer.size()) == 0) {
        g_last_error = p_last_error ? p_last_error() : "Linderaの解析に失敗しました";
        return {};
    }
    return std::string(buffer.data());
}

std::vector<std::string> csv_fields(const std::string& text) {
    std::vector<std::string> out;
    std::string field;
    bool quoted = false;
    for (size_t i = 0; i < text.size(); ++i) {
        const char c = text[i];
        if (c == '"') {
            if (quoted && i + 1 < text.size() && text[i + 1] == '"') {
                field.push_back('"');
                ++i;
            } else {
                quoted = !quoted;
            }
        } else if (c == ',' && !quoted) {
            out.push_back(field);
            field.clear();
        } else {
            field.push_back(c);
        }
    }
    out.push_back(field);
    return out;
}

bool is_katakana(const std::string& text) {
    const auto value = utf8_to_wide(text);
    if (value.empty()) return false;
    for (wchar_t c : value) {
        if (!((c >= L'ァ' && c <= L'ヶ') || c == L'ヴ' || c == L'ー')) return false;
    }
    return true;
}

struct Token {
    std::string surface;
    std::string pos;
    std::string pos1;
    std::string reading;
};

std::vector<Token> parse_output(const std::string& output) {
    std::vector<Token> tokens;
    size_t begin = 0;
    while (begin <= output.size()) {
        const size_t end = output.find('\n', begin);
        std::string line = output.substr(begin, end == std::string::npos ? std::string::npos : end - begin);
        if (!line.empty() && line.back() == '\r') line.pop_back();
        if (!line.empty() && line != "EOS") {
            const auto tab = line.find('\t');
            if (tab != std::string::npos) {
                Token token;
                token.surface = line.substr(0, tab);
                const auto fields = csv_fields(line.substr(tab + 1));
                if (!fields.empty()) token.pos = fields[0];
                if (fields.size() > 1) token.pos1 = fields[1];
                token.reading = token.surface;
                const size_t first = fields.size() > 6 ? 6 : 0;
                for (size_t i = first; i < fields.size(); ++i) {
                    if (fields[i] != "*" && is_katakana(fields[i])) {
                        token.reading = fields[i];
                        break;
                    }
                }
                tokens.push_back(std::move(token));
            }
        }
        if (end == std::string::npos) break;
        begin = end + 1;
    }
    return tokens;
}

bool function_word(const Token& t) {
    return t.pos == "助詞" || t.pos == "助動詞" || t.pos == "記号" || t.pos == "補助記号" ||
           t.pos1 == "接尾" || t.pos1 == "接尾辞" || t.pos1 == "非自立可能";
}

bool prefix_word(const Token& t) {
    return t.pos == "接頭詞" || t.pos == "接頭辞" || t.pos1 == "接頭" || t.pos1 == "接頭辞";
}

std::vector<std::vector<Token>> group_bunsetsu(const std::vector<Token>& tokens) {
    std::vector<std::vector<Token>> groups;
    std::vector<Token> current;
    bool has_content = false;
    for (const auto& token : tokens) {
        const bool functional = function_word(token);
        const bool prefix = prefix_word(token);
        const bool noun_chain = !current.empty() && current.back().pos == "名詞" && token.pos == "名詞";
        if (!current.empty() && has_content && !functional && !noun_chain && !prefix) {
            groups.push_back(std::move(current));
            current.clear();
            has_content = false;
        } else if (!current.empty() && prefix && has_content) {
            groups.push_back(std::move(current));
            current.clear();
            has_content = false;
        }
        current.push_back(token);
        if (!functional && !prefix) has_content = true;
    }
    if (!current.empty()) groups.push_back(std::move(current));
    return groups;
}

const std::unordered_map<std::wstring, std::string>& kana_map() {
    static const std::unordered_map<std::wstring, std::string> map = {
        {L"あ","a"},{L"い","i"},{L"う","u"},{L"え","e"},{L"お","o"},
        {L"か","ka"},{L"き","ki"},{L"く","ku"},{L"け","ke"},{L"こ","ko"},{L"が","ga"},{L"ぎ","gi"},{L"ぐ","gu"},{L"げ","ge"},{L"ご","go"},
        {L"さ","sa"},{L"し","shi"},{L"す","su"},{L"せ","se"},{L"そ","so"},{L"ざ","za"},{L"じ","ji"},{L"ず","zu"},{L"ぜ","ze"},{L"ぞ","zo"},
        {L"た","ta"},{L"ち","chi"},{L"つ","tsu"},{L"て","te"},{L"と","to"},{L"だ","da"},{L"ぢ","di"},{L"づ","du"},{L"で","de"},{L"ど","do"},
        {L"な","na"},{L"に","ni"},{L"ぬ","nu"},{L"ね","ne"},{L"の","no"},{L"は","ha"},{L"ひ","hi"},{L"ふ","fu"},{L"へ","he"},{L"ほ","ho"},
        {L"ば","ba"},{L"び","bi"},{L"ぶ","bu"},{L"べ","be"},{L"ぼ","bo"},{L"ぱ","pa"},{L"ぴ","pi"},{L"ぷ","pu"},{L"ぺ","pe"},{L"ぽ","po"},
        {L"ま","ma"},{L"み","mi"},{L"む","mu"},{L"め","me"},{L"も","mo"},{L"や","ya"},{L"ゆ","yu"},{L"よ","yo"},
        {L"ら","ra"},{L"り","ri"},{L"る","ru"},{L"れ","re"},{L"ろ","ro"},{L"わ","wa"},{L"ゐ","wi"},{L"ゑ","we"},{L"を","wo"},{L"ゔ","vu"},
        {L"きゃ","kya"},{L"きゅ","kyu"},{L"きょ","kyo"},{L"ぎゃ","gya"},{L"ぎゅ","gyu"},{L"ぎょ","gyo"},
        {L"しゃ","sha"},{L"しゅ","shu"},{L"しょ","sho"},{L"じゃ","ja"},{L"じゅ","ju"},{L"じょ","jo"},
        {L"ちゃ","cha"},{L"ちゅ","chu"},{L"ちょ","cho"},{L"にゃ","nya"},{L"にゅ","nyu"},{L"にょ","nyo"},
        {L"ひゃ","hya"},{L"ひゅ","hyu"},{L"ひょ","hyo"},{L"びゃ","bya"},{L"びゅ","byu"},{L"びょ","byo"},{L"ぴゃ","pya"},{L"ぴゅ","pyu"},{L"ぴょ","pyo"},
        {L"みゃ","mya"},{L"みゅ","myu"},{L"みょ","myo"},{L"りゃ","rya"},{L"りゅ","ryu"},{L"りょ","ryo"},
        {L"ふぁ","fa"},{L"ふぃ","fi"},{L"ふぇ","fe"},{L"ふぉ","fo"},{L"うぃ","wi"},{L"うぇ","we"},{L"うぉ","who"},
        {L"しぇ","she"},{L"じぇ","je"},{L"ちぇ","che"},{L"つぁ","tsa"},{L"つぃ","tsi"},{L"つぇ","tse"},{L"つぉ","tso"},
        {L"てぃ","thi"},{L"てゅ","thu"},{L"でぃ","dhi"},{L"でゅ","dhu"},
        {L"ぁ","xa"},{L"ぃ","xi"},{L"ぅ","xu"},{L"ぇ","xe"},{L"ぉ","xo"},{L"ゃ","xya"},{L"ゅ","xyu"},{L"ょ","xyo"},{L"ゎ","xwa"},
        {L"、",","},{L"。","."},{L"！","!"},{L"？","?"},{L"ー","-"},{L" "," "},{L"　"," "},{L"\n","\n"}
    };
    return map;
}

std::string kana_to_romaji(const std::string& input) {
    auto chars = utf8_to_wide(input);
    for (auto& c : chars) if (c >= L'ァ' && c <= L'ヶ') c = static_cast<wchar_t>(c - 0x60);
    const auto& map = kana_map();
    std::string out;
    for (size_t i = 0; i < chars.size();) {
        const wchar_t c = chars[i];
        if (c == L'っ') {
            std::string next;
            if (i + 2 < chars.size()) {
                auto it = map.find(chars.substr(i + 1, 2));
                if (it != map.end()) next = it->second;
            }
            if (next.empty() && i + 1 < chars.size()) {
                auto it = map.find(chars.substr(i + 1, 1));
                if (it != map.end()) next = it->second;
            }
            out += !next.empty() && std::string("aiueon").find(next[0]) == std::string::npos ? std::string(1, next[0]) : "xtu";
            ++i;
            continue;
        }
        if (c == L'ん') {
            std::string next;
            if (i + 2 < chars.size()) {
                auto it = map.find(chars.substr(i + 1, 2));
                if (it != map.end()) next = it->second;
            }
            if (next.empty() && i + 1 < chars.size()) {
                auto it = map.find(chars.substr(i + 1, 1));
                if (it != map.end()) next = it->second;
            }
            out += !next.empty() && std::string("aiueoyn").find(next[0]) != std::string::npos ? "n'" : "n";
            ++i;
            continue;
        }
        if (i + 1 < chars.size()) {
            auto it = map.find(chars.substr(i, 2));
            if (it != map.end()) {
                out += it->second;
                i += 2;
                continue;
            }
        }
        auto it = map.find(chars.substr(i, 1));
        if (it != map.end()) out += it->second;
        else out += wide_to_utf8(chars.substr(i, 1));
        ++i;
    }
    return out;
}

std::string escape_target(const std::string& text) {
    std::string out;
    for (size_t i = 0; i < text.size(); ++i) {
        if (text[i] == '\\' && i + 1 < text.size() && text[i + 1] == '/') {
            out += "\\/";
            ++i;
        } else if (text[i] == '/') {
            out += "\\/";
        } else {
            out.push_back(text[i]);
        }
    }
    return out;
}

std::vector<std::string> split_manual(const std::string& text) {
    std::vector<std::string> parts;
    std::string buffer;
    bool in_tag = false;
    for (size_t i = 0; i < text.size();) {
        if (text.compare(i, 3, "<i>") == 0) {
            buffer += "<i>";
            in_tag = true;
            i += 3;
        } else if (in_tag && (text.compare(i, 4, "</i>") == 0 || text.compare(i, 4, "<i/>") == 0)) {
            buffer += text.substr(i, 4);
            in_tag = false;
            i += 4;
        } else if (text.compare(i, 2, "\\/") == 0) {
            buffer += "\\/";
            i += 2;
        } else if (!in_tag && text[i] == '/') {
            parts.push_back(buffer);
            buffer.clear();
            ++i;
        } else {
            buffer.push_back(text[i++]);
        }
    }
    parts.push_back(buffer);
    return parts;
}

std::string normalize_manual(const std::string& section) {
    if (section.rfind("<i>", 0) != 0) return {};
    size_t close = section.find("</i>", 3);
    if (close == std::string::npos) close = section.find("<i/>", 3);
    if (close == std::string::npos) return {};
    return "<i>" + section.substr(3, close - 3) + "</i>" + escape_target(section.substr(close + 4));
}

std::string analyze_section(const std::string& section, int language) {
    if (section.empty()) return {};
    const auto manual = normalize_manual(section);
    if (!manual.empty()) return manual;

    // 中文の自動解析は次段階。手動の <i>拼音/注音</i> 指定は共通処理で利用できる。
    if (language == 1) return section;

    g_last_error.clear();
    std::string plain = section;
    for (size_t p = 0; (p = plain.find("\/", p)) != std::string::npos;) plain.replace(p, 2, "/");
    const std::string output = lindera_tokenize(plain);
    if (output.empty()) return {};

    std::string result;
    for (const auto& group : group_bunsetsu(parse_output(output))) {
        std::string target;
        std::string reading;
        for (const auto& token : group) {
            target += token.surface;
            reading += token.reading;
        }
        if (!result.empty()) result += '/';
        result += "<i>" + kana_to_romaji(reading) + "</i>" + escape_target(target);
    }
    return result;
}

std::string annotate_text(const std::string& text, int language) {
    const std::string cache_key = std::to_string(language) + "\n" + text;
    auto found = g_cache.find(cache_key);
    if (found != g_cache.end()) return found->second;
    std::string result;
    for (const auto& section : split_manual(text)) {
        const auto annotated = analyze_section(section, language);
        if (!annotated.empty()) {
            if (!result.empty()) result += '/';
            result += annotated;
        }
    }
    if (result.empty()) result = text;
    if (g_cache.size() >= 256) g_cache.clear();
    g_cache.emplace(cache_key, result);
    return result;
}

void annotate(SCRIPT_MODULE_PARAM* param) {
    if (param->get_param_num() < 1 || param->get_param_num() > 2) {
        param->set_error("annotateには文章と言語を指定してください");
        return;
    }
    const char* input = param->get_param_string(0);
    if (!input) {
        param->set_error("文章が文字列ではありません");
        return;
    }
    std::lock_guard<std::mutex> lock(g_mutex);
    const int language = param->get_param_num() >= 2 ? param->get_param_int(1) : 0;
    const auto result = annotate_text(input, language);
    param->push_result_string(result.c_str());
}

void available(SCRIPT_MODULE_PARAM* param) {
    std::lock_guard<std::mutex> lock(g_mutex);
    param->push_result_boolean(load_lindera());
}

void last_error(SCRIPT_MODULE_PARAM* param) {
    std::lock_guard<std::mutex> lock(g_mutex);
    param->push_result_string(g_last_error.c_str());
}

void diagnose(SCRIPT_MODULE_PARAM* param) {
    std::lock_guard<std::mutex> lock(g_mutex);
    g_last_error.clear();
    const auto result = analyze_section("今日は", 0);
    if (result.empty()) {
        std::string message = "NG: " + (g_last_error.empty() ? std::string("Linderaの解析結果が空です") : g_last_error);
        if (!g_lindera_path.empty()) message += "
DLL: " + wide_to_utf8(g_lindera_path);
        param->push_result_string(message.c_str());
        return;
    }
    std::string message = "OK: Linderaで解析できました";
    message += "\nエンジン: Lindera 4.0.0";
    message += "\n辞書: IPADIC（埋め込み）";
    message += "\nDLL: " + wide_to_utf8(g_lindera_path);
    message += "\n結果: " + result;
    param->push_result_string(message.c_str());
}

SCRIPT_MODULE_FUNCTION functions[] = {
    {L"annotate", annotate},
    {L"available", available},
    {L"last_error", last_error},
    {L"diagnose", diagnose},
    {nullptr, nullptr}
};

SCRIPT_MODULE_TABLE table = {
    L"TypingEffect Lindera module v0.4.0",
    functions
};

}  // namespace

BOOL WINAPI DllMain(HINSTANCE instance, DWORD reason, LPVOID) {
    if (reason == DLL_PROCESS_ATTACH) {
        g_self = instance;
        DisableThreadLibraryCalls(instance);
    }
    return TRUE;
}

extern "C" __declspec(dllexport) bool InitializePlugin(DWORD) {
    return true;
}

extern "C" __declspec(dllexport) void UninitializePlugin() {
    std::lock_guard<std::mutex> lock(g_mutex);
    g_cache.clear();
    if (g_lindera_dll) FreeLibrary(g_lindera_dll);
    g_lindera_dll = nullptr;
}

extern "C" __declspec(dllexport) SCRIPT_MODULE_TABLE* GetScriptModuleTable() {
    return &table;
}
