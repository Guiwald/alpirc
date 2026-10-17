//! Le langage : évaluation, identifiants, jetons, conditions, contrôle de flux, variables, alias.

mod common;

use alpirc_script::{Ctx, ScriptError, ScriptKind};
use common::*;

fn e_h() -> (alpirc_script::Engine, FakeHost) {
    (engine(), FakeHost::new())
}

// ---------------------------------------------------------------------------------------------------------------
// Évaluation
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn echo_calc() {
    let (mut e, mut h) = e_h();
    run(&mut e, &mut h, "//echo $calc(1+2*3)");
    assert_eq!(h.last(), ("echo".into(), "7".into()));
}

#[test]
fn calc_variants() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$calc(10/3)"), "3.333333");
    assert_eq!(ev(&mut e, &mut h, "$calc((2+3)*4)"), "20");
    assert_eq!(ev(&mut e, &mut h, "$calc(2^10)"), "1024");
    assert_eq!(ev(&mut e, &mut h, "$calc(7 % 3)"), "1");
    assert_eq!(ev(&mut e, &mut h, "$calc(1 / 0)"), "0");
    assert_eq!(ev(&mut e, &mut h, "$calc(-5 + 2)"), "-3");
}

#[test]
fn single_slash_does_not_evaluate() {
    let (mut e, mut h) = e_h();
    run(&mut e, &mut h, "/echo $me");
    assert_eq!(h.last(), ("echo".into(), "$me".into()));
    run(&mut e, &mut h, "//echo $me");
    assert_eq!(h.last(), ("echo".into(), "Guiwald".into()));
}

#[test]
fn concat_plus() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "a $+ b"), "ab");
    assert_eq!(ev(&mut e, &mut h, "$+(a,b,c)"), "abc");
    assert_eq!(ev(&mut e, &mut h, "$me $+ !"), "Guiwald!");
    assert_eq!(ev(&mut e, &mut h, "x  $+  y"), "xy");
    assert_eq!(ev(&mut e, &mut h, "$len(a $+ b)"), "2");
}

#[test]
fn identifiers_only_at_word_start() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "abc$me"), "abc$me");
    assert_eq!(ev(&mut e, &mut h, "100% sure"), "100% sure");
    assert_eq!(ev(&mut e, &mut h, "cost $ 5"), "cost $ 5");
}

#[test]
fn chars_and_codes() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$chr(65)"), "A");
    assert_eq!(ev(&mut e, &mut h, "$asc(A)"), "65");
    assert_eq!(ev(&mut e, &mut h, "$chr(233)"), "é");
    assert_eq!(ev(&mut e, &mut h, "[$null]"), "[$null]");
    assert_eq!(ev(&mut e, &mut h, "x $null y"), "x  y");
    assert_eq!(ev(&mut e, &mut h, "$len($crlf)"), "2");
    assert_eq!(ev(&mut e, &mut h, "$len($tab)"), "1");
}

#[test]
fn len_unicode() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$len(héllo)"), "5");
    assert_eq!(ev(&mut e, &mut h, "$len(日本語)"), "3");
    assert_eq!(ev(&mut e, &mut h, "$len()"), "0");
}

#[test]
fn left_right_mid() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$left(abcdef,2)"), "ab");
    assert_eq!(ev(&mut e, &mut h, "$left(abcdef,-2)"), "abcd");
    assert_eq!(ev(&mut e, &mut h, "$right(abcdef,2)"), "ef");
    assert_eq!(ev(&mut e, &mut h, "$right(abcdef,-2)"), "cdef");
    assert_eq!(ev(&mut e, &mut h, "$mid(abcdef,2,3)"), "bcd");
    assert_eq!(ev(&mut e, &mut h, "$mid(abcdef,-2)"), "ef");
    assert_eq!(ev(&mut e, &mut h, "$mid(abcdef,2,-1)"), "bcde");
    assert_eq!(ev(&mut e, &mut h, "$left(été,1)"), "é");
}

#[test]
fn pos_and_count() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$pos(hello,l)"), "3");
    assert_eq!(ev(&mut e, &mut h, "$pos(hello,l,2)"), "4");
    assert_eq!(ev(&mut e, &mut h, "$pos(hello,l,0)"), "2");
    assert_eq!(ev(&mut e, &mut h, "$pos(hello,z)"), "");
    assert_eq!(ev(&mut e, &mut h, "$pos(HELLO,l)"), "3");
    assert_eq!(ev(&mut e, &mut h, "$poscs(HELLO,l)"), "");
    assert_eq!(ev(&mut e, &mut h, "$count(banana,a)"), "3");
    assert_eq!(ev(&mut e, &mut h, "$count(banana,an,b)"), "3");
    assert_eq!(ev(&mut e, &mut h, "$countcs(BAnana,a)"), "2");
}

#[test]
fn replace_family() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$replace(abcabc,a,x,b,y)"), "xycxyc");
    assert_eq!(ev(&mut e, &mut h, "$replace(Hello,L,_)"), "He__o");
    assert_eq!(ev(&mut e, &mut h, "$replacecs(Hello,L,_)"), "Hello");
    assert_eq!(ev(&mut e, &mut h, "$replace(ab,a,b,b,c)"), "cc");
    assert_eq!(ev(&mut e, &mut h, "$replacex(ab,a,b,b,c)"), "bc");
    assert_eq!(ev(&mut e, &mut h, "$remove(hello world,o,l)"), "he wrd");
    assert_eq!(ev(&mut e, &mut h, "$removecs(HeLlo,l)"), "HeLo");
}

#[test]
fn case_and_repeat() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$upper(abc)"), "ABC");
    assert_eq!(ev(&mut e, &mut h, "$lower(ÀBC)"), "àbc");
    assert_eq!(ev(&mut e, &mut h, "$str(ab,3)"), "ababab");
    assert_eq!(ev(&mut e, &mut h, "$isupper(ABC)"), "$true");
    assert_eq!(ev(&mut e, &mut h, "$islower(ABC)"), "$false");
}

#[test]
fn strip_codes() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$strip(\u{2}bold\u{2} \u{3}4red\u{3} \u{1f}u\u{1f})"), "bold red u");
}

#[test]
fn quotes_and_ordinals() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$qt(a b)"), "\"a b\"");
    assert_eq!(ev(&mut e, &mut h, "$noqt(\"a b\")"), "a b");
    assert_eq!(ev(&mut e, &mut h, "$ord(1) $ord(12) $ord(22) $ord(103)"), "1st 12th 22nd 103rd");
}

#[test]
fn base_and_bytes() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$base(255,10,16)"), "FF");
    assert_eq!(ev(&mut e, &mut h, "$base(FF,16,10)"), "255");
    assert_eq!(ev(&mut e, &mut h, "$base(5,10,2,8)"), "00000101");
    assert_eq!(ev(&mut e, &mut h, "$bytes(1234567)"), "1,234,567");
    assert_eq!(ev(&mut e, &mut h, "$bytes(1536,k)"), "1.50");
    assert_eq!(ev(&mut e, &mut h, "$bytes(1234567,3).suf"), "1.18MB");
}

#[test]
fn duration_both_ways() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$duration(3700)"), "1hr 1min 40secs");
    assert_eq!(ev(&mut e, &mut h, "$duration(3700,3)"), "01:01:40");
    assert_eq!(ev(&mut e, &mut h, "$duration(1hr 1min 40secs)"), "3700");
    assert_eq!(ev(&mut e, &mut h, "$duration(90061)"), "1day 1hr 1min 1sec");
}

#[test]
fn asctime_formats() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$asctime(1000000000,yyyy-mm-dd HH:nn:ss)"), "2001-09-09 01:46:40");
    assert_eq!(ev(&mut e, &mut h, "$asctime(1000000000)"), "Sun Sep 09 01:46:40 2001");
    assert_eq!(ev(&mut e, &mut h, "$asctime(1000000000,dddd d mmmm yyyy)"), "Sunday 9 September 2001");
    assert_eq!(ev(&mut e, &mut h, "$asctime(1000000000,hh:nn tt)"), "01:46 am");
    h.offset = 3600;
    assert_eq!(ev(&mut e, &mut h, "$asctime(1000000000,HH:nn zz)"), "02:46 +0100");
}

#[test]
fn now_identifiers() {
    let (mut e, mut h) = e_h();
    // 1700000000 = mardi 14 novembre 2023, 22:13:20 UTC
    assert_eq!(ev(&mut e, &mut h, "$ctime"), "1700000000");
    assert_eq!(ev(&mut e, &mut h, "$time"), "22:13:20");
    assert_eq!(ev(&mut e, &mut h, "$date"), "14/11/2023");
    assert_eq!(ev(&mut e, &mut h, "$day"), "Tuesday");
    assert_eq!(ev(&mut e, &mut h, "$fulldate"), "Tue Nov 14 22:13:20 2023");
    assert_eq!(ev(&mut e, &mut h, "$time(HH:nn)"), "22:13");
    assert_eq!(ev(&mut e, &mut h, "$gmt"), "1700000000");
    assert_eq!(ev(&mut e, &mut h, "$ticks"), "1700000000000");
    h.offset = 3600;
    assert_eq!(ev(&mut e, &mut h, "$timezone"), "-3600");
    assert_eq!(ev(&mut e, &mut h, "$time"), "23:13:20");
}

#[test]
fn ctime_parse() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$ctime(09/09/2001 01:46:40)"), "1000000000");
    assert_eq!(ev(&mut e, &mut h, "$ctime(2001-09-09 01:46:40)"), "1000000000");
    assert_eq!(ev(&mut e, &mut h, "$ctime($asctime(1000000000))"), "1000000000");
}

#[test]
fn hashes_and_base64() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$md5(abc)"), "900150983cd24fb0d6963f7d28e17f72");
    assert_eq!(ev(&mut e, &mut h, "$sha1(abc)"), "a9993e364706816aba3e25717850c26c9cd0d89d");
    assert_eq!(ev(&mut e, &mut h, "$sha256(abc)"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    assert_eq!(ev(&mut e, &mut h, "$crc(abc,0)"), "352441C2");
    assert_eq!(ev(&mut e, &mut h, "$encode(hello,m)"), "aGVsbG8=");
    assert_eq!(ev(&mut e, &mut h, "$decode(aGVsbG8=,m)"), "hello");
}

#[test]
fn random_ranges() {
    let (mut e, mut h) = e_h();
    for _ in 0..50 {
        let v: i64 = ev(&mut e, &mut h, "$rand(1,6)").parse().unwrap();
        assert!((1..=6).contains(&v));
        let l = ev(&mut e, &mut h, "$r(a,c)");
        assert!(["a", "b", "c"].contains(&l.as_str()), "{l}");
    }
}

#[test]
fn math_identifiers() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$round(3.14159,2)"), "3.14");
    assert_eq!(ev(&mut e, &mut h, "$int(3.9)"), "3");
    assert_eq!(ev(&mut e, &mut h, "$abs(-5)"), "5");
    assert_eq!(ev(&mut e, &mut h, "$sqrt(16)"), "4");
    assert_eq!(ev(&mut e, &mut h, "$floor(2.7) $ceil(2.1)"), "2 3");
    assert_eq!(ev(&mut e, &mut h, "$min(3,1,2) $max(3,1,2)"), "1 3");
    assert_eq!(ev(&mut e, &mut h, "$sin(90).deg"), "1");
    assert_eq!(ev(&mut e, &mut h, "$cos(0)"), "1");
    assert_eq!(ev(&mut e, &mut h, "$pi"), "3.14159265358979323846");
}

#[test]
fn iif() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$iif(1 == 1,yes,no)"), "yes");
    assert_eq!(ev(&mut e, &mut h, "$iif(1 == 2,yes,no)"), "no");
    assert_eq!(ev(&mut e, &mut h, "$iif($null,yes)"), "");
    assert_eq!(ev(&mut e, &mut h, "$iif($me == Guiwald,$upper(me),x)"), "ME");
}

#[test]
fn eval_levels() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$eval($me,0)"), "$me");
    assert_eq!(ev(&mut e, &mut h, "$($me,0)"), "$me");
    run(&mut e, &mut h, "/set %code $me");
    assert_eq!(ev(&mut e, &mut h, "%code"), "$me");
    assert_eq!(ev(&mut e, &mut h, "$eval(%code,1)"), "$me");
    assert_eq!(ev(&mut e, &mut h, "$eval(%code,2)"), "Guiwald");
    assert_eq!(ev(&mut e, &mut h, "$(%code,2)"), "Guiwald");
}

#[test]
fn delayed_identifier() {
    let (mut e, mut h) = e_h();
    run(&mut e, &mut h, "//echo $!me $!calc(1+2) $!1-");
    assert_eq!(h.last().1, "$me $calc(1+2) $1-");
}

#[test]
fn halt_on_empty_double_dollar() {
    let (mut e, mut h) = setup("alias t { echo -a start | echo -a $$1 | echo -a end }");
    run(&mut e, &mut h, "/t");
    assert_eq!(h.says(), vec!["start"]);
    h.clear();
    run(&mut e, &mut h, "/t x");
    assert_eq!(h.says(), vec!["start", "x", "end"]);
}

#[test]
fn evaluation_brackets_build_names() {
    let (mut e, mut h) = e_h();
    run(&mut e, &mut h, "/set %a.Guiwald 5");
    assert_eq!(ev(&mut e, &mut h, "%a. [ $+ [ $me ] ]"), "5");
    run(&mut e, &mut h, "//set %b. [ $+ [ $me ] ] 7");
    assert_eq!(ev(&mut e, &mut h, "%b.Guiwald"), "7");
    run(&mut e, &mut h, "//inc %b. [ $+ [ $me ] ]");
    assert_eq!(ev(&mut e, &mut h, "%b.Guiwald"), "8");
}

#[test]
fn evaluation_brackets_double_eval() {
    let (mut e, mut h) = e_h();
    run(&mut e, &mut h, "/set %code $me");
    assert_eq!(ev(&mut e, &mut h, "[ %code ]"), "Guiwald");
    assert_eq!(ev(&mut e, &mut h, "%code"), "$me");
    // Crochet non fermé : du texte.
    assert_eq!(ev(&mut e, &mut h, "[ abc"), "[ abc");
}

#[test]
fn params_and_ranges() {
    let (mut e, mut h) = setup("alias t echo -a $0 $1 $2- $2-3 < $+ $5 $+ >");
    run(&mut e, &mut h, "/t a b c d");
    assert_eq!(h.says(), vec!["4 a b c d b c <>"]);
}

#[test]
fn star_loops_over_tokens() {
    let (mut e, mut h) = e_h();
    run(&mut e, &mut h, "//tokenize 32 a b c | echo -a $*");
    assert_eq!(h.says(), vec!["a", "b", "c"]);
}

#[test]
fn tokenize_sets_params() {
    let (mut e, mut h) = e_h();
    run(&mut e, &mut h, "//tokenize 44 a,b,,c | echo -a $0 $2 $3");
    assert_eq!(h.says(), vec!["3 b c"]);
}

#[test]
fn host_identifiers_and_unknown() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$server"), "irc.test");
    assert_eq!(ev(&mut e, &mut h, "<$nosuchthing>"), "<$nosuchthing>");
    assert_eq!(ev(&mut e, &mut h, "a $nosuchthing b"), "a  b");
    assert_eq!(ev(&mut e, &mut h, "$version"), "AlpIRC");
    h.idents.insert("version".into(), "1.0".into());
    assert_eq!(ev(&mut e, &mut h, "$version"), "1.0");
}

#[test]
fn identifier_followed_by_text() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$me's"), "Guiwald's");
    assert_eq!(ev(&mut e, &mut h, "$upper(a)b"), "Ab");
}

#[test]
fn mask_identifier() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$mask(nick!user@host.example.com,2)"), "*!*@host.example.com");
    assert_eq!(ev(&mut e, &mut h, "$mask(n!~u@a.b.fr,3)"), "*!*u@*.b.fr");
    assert_eq!(ev(&mut e, &mut h, "$mask(n!~u@a.b.fr,5)"), "n!~u@a.b.fr");
}

#[test]
fn calc_with_vars_in_parens() {
    let (mut e, mut h) = e_h();
    run(&mut e, &mut h, "/set %x 2");
    assert_eq!(ev(&mut e, &mut h, "$calc((1 + %x) * 2)"), "6");
    assert_eq!(ev(&mut e, &mut h, "$len(%x)"), "1");
    assert_eq!(ev(&mut e, &mut h, "$calc(%x * 3)"), "6");
}

#[test]
fn nested_identifiers() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$upper($left($replace(hello world,o,0),7))"), "HELL0 W");
    assert_eq!(ev(&mut e, &mut h, "$gettok($gettok(a.b-c.d,2,46),1,45)"), "b");
}

#[test]
fn prompt_identifier() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$?=\"Your name?\""), "answer");
    assert_eq!(ev(&mut e, &mut h, "$?"), "answer");
}

#[test]
fn snicks_and_selected_params() {
    let (mut e, mut h) = e_h();
    let c = Ctx { selected: vec!["Bob".into(), "Carl".into()], ..ctx() };
    assert_eq!(e.evaluate(&mut h, &c, "$snicks / $1 / $2 / $0"), "Bob,Carl / Bob / Carl / 2");
}

#[test]
fn chan_from_window() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$chan"), "#test");
    assert_eq!(ev(&mut e, &mut h, "$active"), "#test");
    assert_eq!(ev(&mut e, &mut h, "$cid"), "1");
}

// ---------------------------------------------------------------------------------------------------------------
// Jetons
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn gettok_family() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$gettok(a.b.c,2,46)"), "b");
    assert_eq!(ev(&mut e, &mut h, "$gettok(a.b.c,2-,46)"), "b.c");
    assert_eq!(ev(&mut e, &mut h, "$gettok(a.b.c,-1,46)"), "c");
    assert_eq!(ev(&mut e, &mut h, "$gettok(a.b.c,0,46)"), "3");
    assert_eq!(ev(&mut e, &mut h, "$gettok(a.b.c,1-2,46)"), "a.b");
    assert_eq!(ev(&mut e, &mut h, "$gettok(a b c,9,32)"), "");
    assert_eq!(ev(&mut e, &mut h, "$numtok(a b c,32)"), "3");
    assert_eq!(ev(&mut e, &mut h, "$numtok(,32)"), "0");
}

#[test]
fn addtok_deltok_istok() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$addtok(a.b,c,46)"), "a.b.c");
    assert_eq!(ev(&mut e, &mut h, "$addtok(a.b,B,46)"), "a.b");
    assert_eq!(ev(&mut e, &mut h, "$addtokcs(a.b,B,46)"), "a.b.B");
    assert_eq!(ev(&mut e, &mut h, "$addtok($null,a,46)"), "a");
    assert_eq!(ev(&mut e, &mut h, "$deltok(a.b.c,2,46)"), "a.c");
    assert_eq!(ev(&mut e, &mut h, "$deltok(a.b.c.d,2-3,46)"), "a.d");
    assert_eq!(ev(&mut e, &mut h, "$istok(a.b.c,B,46)"), "$true");
    assert_eq!(ev(&mut e, &mut h, "$istokcs(a.b.c,B,46)"), "$false");
}

#[test]
fn findtok_instok_puttok() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$findtok(a.b.c.b,b,2,46)"), "4");
    assert_eq!(ev(&mut e, &mut h, "$findtok(a.b.c.b,b,0,46)"), "2");
    assert_eq!(ev(&mut e, &mut h, "$findtok(a.b.c,z,1,46)"), "");
    assert_eq!(ev(&mut e, &mut h, "$instok(a.c,b,2,46)"), "a.b.c");
    assert_eq!(ev(&mut e, &mut h, "$instok(a.b,c,9,46)"), "a.b.c");
    assert_eq!(ev(&mut e, &mut h, "$puttok(a.x.c,b,2,46)"), "a.b.c");
}

#[test]
fn reptok_remtok() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$reptok(a.b.a,a,z,46)"), "z.b.a");
    assert_eq!(ev(&mut e, &mut h, "$reptok(a.b.a,a,z,2,46)"), "a.b.z");
    assert_eq!(ev(&mut e, &mut h, "$remtok(a.b.a,a,1,46)"), "b.a");
    assert_eq!(ev(&mut e, &mut h, "$remtok(a.b.a,a,0,46)"), "b");
    assert_eq!(ev(&mut e, &mut h, "$remtok(a.b.a,a,46)"), "b.a");
}

#[test]
fn matchtok_wildtok() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$matchtok(one two three,o,0,32)"), "2");
    assert_eq!(ev(&mut e, &mut h, "$matchtok(one two three,o,2,32)"), "two");
    assert_eq!(ev(&mut e, &mut h, "$wildtok(one two three,t*,0,32)"), "2");
    assert_eq!(ev(&mut e, &mut h, "$wildtok(one two three,t*,2,32)"), "three");
    assert_eq!(ev(&mut e, &mut h, "$wildtok(one two three,T*,1,32)"), "two");
    assert_eq!(ev(&mut e, &mut h, "$wildtokcs(one two three,T*,1,32)"), "");
}

#[test]
fn sorttok() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$sorttok(c a B,32)"), "a B c");
    assert_eq!(ev(&mut e, &mut h, "$sorttok(10 9 100,32,n)"), "9 10 100");
    assert_eq!(ev(&mut e, &mut h, "$sorttok(a b c,32,r)"), "c b a");
    assert_eq!(ev(&mut e, &mut h, "$sorttok(bob @al +cy,32,c)"), "@al +cy bob");
}

#[test]
fn tokens_with_comma_separator() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$gettok(a $+ $chr(44) $+ b,2,44)"), "b");
    run(&mut e, &mut h, "/set %list x,y,z");
    assert_eq!(ev(&mut e, &mut h, "$gettok(%list,-1,44)"), "z");
    assert_eq!(ev(&mut e, &mut h, "$numtok(%list,44)"), "3");
}

#[test]
fn invalid_parameters_error() {
    let (mut e, mut h) = e_h();
    let err = run_err(&mut e, &mut h, "//echo $gettok(a.b)");
    assert!(err.contains("Invalid parameters: $gettok"), "{err}");
    let err = run_err(&mut e, &mut h, "//echo $left(abc)");
    assert!(err.contains("$left"), "{err}");
}

// ---------------------------------------------------------------------------------------------------------------
// Expressions régulières
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn regex_and_regml() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$regex(abc,/(b)/)"), "1");
    assert_eq!(ev(&mut e, &mut h, "$regml(1)"), "b");
    assert_eq!(ev(&mut e, &mut h, "$regml(1).pos"), "2");
    assert_eq!(ev(&mut e, &mut h, "$regex(ABC,/b/)"), "0");
    assert_eq!(ev(&mut e, &mut h, "$regex(ABC,/b/i)"), "1");
    assert_eq!(ev(&mut e, &mut h, "$regex(abab,/b/g)"), "2");
    assert_eq!(ev(&mut e, &mut h, "$regml(0)"), "2");
}

#[test]
fn regex_named() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$regex(n,xyz,/(y)(z)/)"), "1");
    assert_eq!(ev(&mut e, &mut h, "$regml(n,2)"), "z");
    assert_eq!(ev(&mut e, &mut h, "$regml(n,0)"), "2");
    assert_eq!(ev(&mut e, &mut h, "$regml(0)"), "0");
}

#[test]
fn regsub_and_regsubex() {
    let (mut e, mut h) = e_h();
    assert_eq!(ev(&mut e, &mut h, "$regsub(hello world,/o/g,0,%r)"), "2");
    assert_eq!(ev(&mut e, &mut h, "%r"), "hell0 w0rld");
    assert_eq!(ev(&mut e, &mut h, "$regsub(ab,/(a)(b)/,\\2\\1,%r)"), "1");
    assert_eq!(ev(&mut e, &mut h, "%r"), "ba");
    assert_eq!(ev(&mut e, &mut h, "$regsubex(abc,/(.)/g,\\1-)"), "a-b-c-");
    assert_eq!(ev(&mut e, &mut h, "$regsubex(abc,/(b)/,$upper(\\1))"), "aBc");
}

#[test]
fn regex_bad_pattern_is_error() {
    let (mut e, mut h) = e_h();
    let err = run_err(&mut e, &mut h, "//echo $regex(a,/(/)");
    assert!(err.contains("$regex"), "{err}");
}

// ---------------------------------------------------------------------------------------------------------------
// Conditions
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn cond_equality() {
    let (mut e, mut h) = e_h();
    assert!(cond(&mut e, &mut h, "1 == 1.0"));
    assert!(cond(&mut e, &mut h, "abc == ABC"));
    assert!(!cond(&mut e, &mut h, "abc === ABC"));
    assert!(cond(&mut e, &mut h, "abc === abc"));
    assert!(cond(&mut e, &mut h, "a != b"));
    assert!(!cond(&mut e, &mut h, "a != A"));
    assert!(cond(&mut e, &mut h, "$null == $null"));
    assert!(cond(&mut e, &mut h, "hello world == hello world"));
}

#[test]
fn cond_ordering() {
    let (mut e, mut h) = e_h();
    assert!(cond(&mut e, &mut h, "10 > 9"));
    assert!(cond(&mut e, &mut h, "9 < 10"));
    assert!(cond(&mut e, &mut h, "b > a"));
    assert!(cond(&mut e, &mut h, "5 >= 5"));
    assert!(cond(&mut e, &mut h, "5 <= 6"));
    assert!(!cond(&mut e, &mut h, "-1 > 0"));
}

#[test]
fn cond_multiples_and_bits() {
    let (mut e, mut h) = e_h();
    assert!(cond(&mut e, &mut h, "3 // 9"));
    assert!(!cond(&mut e, &mut h, "2 // 9"));
    assert!(cond(&mut e, &mut h, "2 \\\\ 9"));
    assert!(cond(&mut e, &mut h, "5 & 4"));
    assert!(!cond(&mut e, &mut h, "2 & 4"));
}

#[test]
fn cond_text_operators() {
    let (mut e, mut h) = e_h();
    assert!(cond(&mut e, &mut h, "ell isin hello"));
    assert!(cond(&mut e, &mut h, "ELL isin hello"));
    assert!(!cond(&mut e, &mut h, "ELL isincs hello"));
    assert!(cond(&mut e, &mut h, "h*o iswm hello"));
    assert!(cond(&mut e, &mut h, "h?llo iswm HELLO"));
    assert!(!cond(&mut e, &mut h, "H*O iswmcs hello"));
    assert!(cond(&mut e, &mut h, "x !isin abc"));
    assert!(cond(&mut e, &mut h, "z* !iswm abc"));
    run(&mut e, &mut h, "/.set %pat !seen &");
    assert!(cond(&mut e, &mut h, "%pat iswm !seen bob"));
    assert!(!cond(&mut e, &mut h, "%pat iswm !seen bob alice"));
}

#[test]
fn cond_numbers_and_letters() {
    let (mut e, mut h) = e_h();
    assert!(cond(&mut e, &mut h, "3 isnum 1-5"));
    assert!(!cond(&mut e, &mut h, "6 isnum 1-5"));
    assert!(cond(&mut e, &mut h, "6 isnum 5-"));
    assert!(!cond(&mut e, &mut h, "abc isnum"));
    assert!(cond(&mut e, &mut h, "-2.5 isnum"));
    assert!(cond(&mut e, &mut h, "a isletter"));
    assert!(cond(&mut e, &mut h, "a isletter abc"));
    assert!(!cond(&mut e, &mut h, "d isletter abc"));
    assert!(cond(&mut e, &mut h, "abc123 isalnum"));
    assert!(cond(&mut e, &mut h, "abc isalpha"));
    assert!(!cond(&mut e, &mut h, "ab1 isalpha"));
    assert!(cond(&mut e, &mut h, "abc islower"));
    assert!(cond(&mut e, &mut h, "ABC isupper"));
    assert!(!cond(&mut e, &mut h, "AbC isupper"));
}

#[test]
fn cond_channel_operators() {
    let (mut e, mut h) = e_h();
    assert!(cond(&mut e, &mut h, "Bob ison #test"));
    assert!(cond(&mut e, &mut h, "bob isop #TEST"));
    assert!(!cond(&mut e, &mut h, "Carl isop #test"));
    assert!(cond(&mut e, &mut h, "Carl isvoice #test"));
    assert!(cond(&mut e, &mut h, "Hal ishop #test"));
    assert!(cond(&mut e, &mut h, "Dave isreg #test"));
    assert!(!cond(&mut e, &mut h, "Bob isreg #test"));
    assert!(!cond(&mut e, &mut h, "Zed ison #test"));
    assert!(cond(&mut e, &mut h, "Zed !ison #test"));
    assert!(cond(&mut e, &mut h, "#test ischan"));
    assert!(!cond(&mut e, &mut h, "bob ischan"));
    assert!(cond(&mut e, &mut h, "troll isignore"));
    assert!(!cond(&mut e, &mut h, "friend isignore"));
}

#[test]
fn cond_logic() {
    let (mut e, mut h) = e_h();
    assert!(cond(&mut e, &mut h, "(1 == 1) && (2 == 2)"));
    assert!(cond(&mut e, &mut h, "1 == 2 || 2 == 2"));
    assert!(!cond(&mut e, &mut h, "1 == 2 && 2 == 2"));
    assert!(cond(&mut e, &mut h, "(1 == 2) || (3 == 3 && 4 == 4)"));
    assert!(cond(&mut e, &mut h, "!(1 == 2)"));
    assert!(cond(&mut e, &mut h, "((1 == 1))"));
}

#[test]
fn cond_values() {
    let (mut e, mut h) = e_h();
    assert!(!cond(&mut e, &mut h, "%undefined"));
    assert!(cond(&mut e, &mut h, "!%undefined"));
    assert!(!cond(&mut e, &mut h, "0"));
    assert!(!cond(&mut e, &mut h, "$false"));
    assert!(cond(&mut e, &mut h, "abc"));
    assert!(cond(&mut e, &mut h, "$true"));
    assert!(cond(&mut e, &mut h, "$len(a b) == 3"));
}

#[test]
fn v1_v2_ifmatch() {
    let (mut e, mut h) = e_h();
    run(&mut e, &mut h, "//if (abc == def) noop | echo -a $v1 $v2 $ifmatch");
    assert_eq!(h.says(), vec!["abc def abc"]);
}

#[test]
fn short_circuit() {
    let (mut e, mut h) = setup("alias side { echo -a side | return 1 }");
    run(&mut e, &mut h, "//if (1 == 2 && $side) echo -a no");
    run(&mut e, &mut h, "//if (1 == 1 || $side) echo -a yes");
    assert_eq!(h.says(), vec!["yes"]);
}

// ---------------------------------------------------------------------------------------------------------------
// Contrôle de flux
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn if_elseif_else() {
    let (mut e, mut h) =
        setup("alias t {\n  if ($1 == 1) { echo -a one }\n  elseif ($1 == 2) { echo -a two }\n  else { echo -a other }\n}");
    run(&mut e, &mut h, "/t 1");
    run(&mut e, &mut h, "/t 2");
    run(&mut e, &mut h, "/t 3");
    assert_eq!(h.says(), vec!["one", "two", "other"]);
}

#[test]
fn inline_if_and_pipes() {
    let (mut e, mut h) = e_h();
    run(&mut e, &mut h, "//if (1 == 1) echo -a yes | echo -a after");
    run(&mut e, &mut h, "//if (1 == 2) echo -a yes | else echo -a no");
    run(&mut e, &mut h, "//if (1 == 2) { echo -a a } | else if (2 == 2) { echo -a b } | else echo -a c");
    assert_eq!(h.says(), vec!["yes", "after", "no", "b"]);
}

#[test]
fn while_loop() {
    let (mut e, mut h) = e_h();
    run(&mut e, &mut h, "//var %i = 1 | while (%i <= 3) { echo -a %i | inc %i }");
    assert_eq!(h.says(), vec!["1", "2", "3"]);
}

#[test]
fn break_and_continue() {
    let (mut e, mut h) = setup(
        "alias t {\n  var %i = 0\n  while (%i < 10) {\n    inc %i\n    if (%i == 2) continue\n    if (%i == 4) break\n    echo -a %i\n  }\n  echo -a end %i\n}",
    );
    run(&mut e, &mut h, "/t");
    assert_eq!(h.says(), vec!["1", "3", "end 4"]);
}

#[test]
fn goto_loop() {
    let (mut e, mut h) = setup("alias t {\n  var %i = 0\n  :loop\n  inc %i\n  if (%i < 3) goto loop\n  echo -a done %i\n}");
    run(&mut e, &mut h, "/t");
    assert_eq!(h.says(), vec!["done 3"]);
}

#[test]
fn goto_out_of_block_and_missing_label() {
    let (mut e, mut h) = setup(
        "alias t {\n  if (1) {\n    goto fin\n  }\n  echo -a skipped\n  :fin\n  echo -a fin\n}\nalias bad { goto nowhere }",
    );
    run(&mut e, &mut h, "/t");
    assert_eq!(h.says(), vec!["fin"]);
    let err = run_err(&mut e, &mut h, "/bad");
    assert!(err.contains("'nowhere' not found"), "{err}");
}

#[test]
fn nested_blocks() {
    let (mut e, mut h) = setup(
        "alias t {\n  var %i = 1\n  while (%i <= 2) {\n    var %j = 1\n    while (%j <= 2) {\n      if (%i == %j) { echo -a same %i }\n      else { echo -a diff %i %j }\n      inc %j\n    }\n    inc %i\n  }\n}",
    );
    run(&mut e, &mut h, "/t");
    assert_eq!(h.says(), vec!["same 1", "diff 1 2", "diff 2 1", "same 2"]);
}

#[test]
fn comments_are_ignored() {
    let (mut e, mut h) = setup(
        "; commentaire { avec accolade\nalias t {\n  ; dedans aussi }\n  echo -a ok | ; fin de ligne }\n}\n/*\nalias t2 { echo -a jamais }\n*/\nalias t3 echo -a t3",
    );
    run(&mut e, &mut h, "/t");
    run(&mut e, &mut h, "/t2");
    run(&mut e, &mut h, "/t3");
    assert_eq!(h.says(), vec!["ok", "t3"]);
    assert!(h.find("t2").len() == 1, "t2 n'est pas un alias : passe à l'hôte");
}

#[test]
fn halt_stops_callers() {
    let (mut e, mut h) = setup("alias a { b | echo -a after }\nalias b { echo -a in b | halt }");
    run(&mut e, &mut h, "/a");
    assert_eq!(h.says(), vec!["in b"]);
}

#[test]
fn return_stops_alias() {
    let (mut e, mut h) = setup("alias a { echo -a one | return | echo -a two }");
    run(&mut e, &mut h, "/a");
    assert_eq!(h.says(), vec!["one"]);
}

#[test]
fn line_continuation() {
    let (mut e, mut h) = setup("alias t {\n  echo -a one $&\n    two $&\n    three\n}");
    run(&mut e, &mut h, "/t");
    assert_eq!(h.says(), vec!["one two three"]);
}

#[test]
fn infinite_loop_guard_default() {
    let (mut e, mut h) = e_h();
    let err = run_err(&mut e, &mut h, "//while (1) { noop }");
    assert!(err.contains("script too deep / infinite loop"), "{err}");
}

#[test]
fn infinite_loop_guard_goto() {
    let (mut e, mut h) = setup("alias t {\n  :a\n  goto a\n}");
    e.set_limits(5_000, 100);
    let err = run_err(&mut e, &mut h, "/t");
    assert!(err.contains("infinite loop"), "{err}");
}

#[test]
fn recursion_guard() {
    let r = std::thread::Builder::new()
        .stack_size(8 << 20)
        .spawn(|| {
            let (mut e, mut h) = setup("alias rec return $rec\nalias ping2 { pong2 }\nalias pong2 { ping2 }");
            let err = run_err(&mut e, &mut h, "//echo -a $rec");
            assert!(err.contains("too deep"), "{err}");
            let err = run_err(&mut e, &mut h, "/ping2");
            assert!(err.contains("too deep"), "{err}");
        })
        .unwrap()
        .join();
    assert!(r.is_ok());
}

// ---------------------------------------------------------------------------------------------------------------
// Variables
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn set_unset_global() {
    let (mut e, mut h) = e_h();
    run(&mut e, &mut h, "/set %x 5");
    assert_eq!(h.echoes(), vec!["* Set %x to 5"]);
    assert!(e.vars_text().contains("%x 5"));
    run(&mut e, &mut h, "/.set %y 6");
    assert_eq!(h.echoes().len(), 1);
    run(&mut e, &mut h, "/unset %x");
    assert!(!e.vars_text().contains("%x"));
}

#[test]
fn set_evaluates_with_double_slash() {
    let (mut e, mut h) = e_h();
    run(&mut e, &mut h, "//set %y $me");
    run(&mut e, &mut h, "/set %z $me");
    assert_eq!(ev(&mut e, &mut h, "%y %z"), "Guiwald $me");
}

#[test]
fn set_in_script_is_silent() {
    let (mut e, mut h) = setup("alias t { set %q 1 | inc %q | echo -a %q }");
    run(&mut e, &mut h, "/t");
    assert_eq!(h.echoes(), vec!["2"]);
}

#[test]
fn locals_shadow_globals() {
    let (mut e, mut h) = setup("alias t { var %x = local | echo -a %x | u }\nalias u { echo -a u= $+ %x }");
    run(&mut e, &mut h, "/.set %x global");
    run(&mut e, &mut h, "/t");
    assert_eq!(h.says(), vec!["local", "u=global"]);
    assert_eq!(ev(&mut e, &mut h, "%x"), "global");
}

#[test]
fn var_multiple_and_math() {
    let (mut e, mut h) = setup(
        "alias t { var %a = 1, %b = $gettok(x.y,2,46), %c | echo -a %a %b < $+ %c $+ > }\nalias m { var %a = 2 * 3 | var -n %b = 2 * 3 | echo -a %a / %b }\nalias s { var %a 7 | echo -a %a }",
    );
    run(&mut e, &mut h, "/t");
    run(&mut e, &mut h, "/m");
    run(&mut e, &mut h, "/s");
    assert_eq!(h.says(), vec!["1 y <>", "6 / 2 * 3", "7"]);
}

#[test]
fn var_global_switch() {
    let (mut e, mut h) = setup("alias t { var -g %gv = here }");
    run(&mut e, &mut h, "/t");
    assert_eq!(ev(&mut e, &mut h, "%gv"), "here");
}

#[test]
fn set_simple_math() {
    let (mut e, mut h) = e_h();
    run(&mut e, &mut h, "/.set %m 5 + 3");
    run(&mut e, &mut h, "/.set -n %n 5 + 3");
    run(&mut e, &mut h, "/.set %o 5+3");
    assert_eq!(ev(&mut e, &mut h, "%m / %n / %o"), "8 / 5 + 3 / 5+3");
}

#[test]
fn inc_dec() {
    let (mut e, mut h) = e_h();
    run(&mut e, &mut h, "/.set %c 5");
    run(&mut e, &mut h, "/.inc %c");
    run(&mut e, &mut h, "/.inc %c 10");
    run(&mut e, &mut h, "/.dec %c 2");
    assert_eq!(ev(&mut e, &mut h, "%c"), "14");
    run(&mut e, &mut h, "/.inc %new");
    assert_eq!(ev(&mut e, &mut h, "%new"), "1");
    run(&mut e, &mut h, "/.dec %f 0.5");
    assert_eq!(ev(&mut e, &mut h, "%f"), "-0.5");
}

#[test]
fn set_u_expires_on_tick() {
    let (mut e, mut h) = e_h();
    run(&mut e, &mut h, "/.set -u10 %t hi");
    let start = h.now;
    assert_eq!(e.next_timer_due(), Some(start + 10_000));
    e.tick(&mut h, start + 5_000);
    assert_eq!(ev(&mut e, &mut h, "%t"), "hi");
    e.tick(&mut h, start + 10_001);
    assert_eq!(ev(&mut e, &mut h, "%t"), "");
}

#[test]
fn set_u0_unsets_at_script_end() {
    let (mut e, mut h) = setup("alias t { set -u0 %tmp x | echo -a %tmp }");
    run(&mut e, &mut h, "/t");
    assert_eq!(h.says(), vec!["x"]);
    assert_eq!(ev(&mut e, &mut h, "%tmp"), "");
}

#[test]
fn inc_u_flood_counter() {
    let (mut e, mut h) = e_h();
    run(&mut e, &mut h, "/.inc -u5 %flood");
    run(&mut e, &mut h, "/.inc -u5 %flood");
    assert_eq!(ev(&mut e, &mut h, "%flood"), "2");
    let t = h.now + 5_000;
    e.tick(&mut h, t);
    assert_eq!(ev(&mut e, &mut h, "%flood"), "");
}

#[test]
fn inc_continuous() {
    let (mut e, mut h) = e_h();
    run(&mut e, &mut h, "/.inc -c %cc");
    assert_eq!(ev(&mut e, &mut h, "%cc"), "1");
    let t0 = h.now;
    e.tick(&mut h, t0 + 1_000);
    assert_eq!(ev(&mut e, &mut h, "%cc"), "2");
    e.tick(&mut h, t0 + 3_000);
    assert_eq!(ev(&mut e, &mut h, "%cc"), "4");
}

#[test]
fn unset_wildcards_and_unsetall() {
    let (mut e, mut h) = e_h();
    run(&mut e, &mut h, "/.set %a1 1 | .set %a2 2 | .set %b 3");
    run(&mut e, &mut h, "/.unset %a*");
    assert_eq!(e.vars_text(), "%b 3\n");
    run(&mut e, &mut h, "/.unsetall");
    assert_eq!(e.vars_text(), "");
}

#[test]
fn vars_text_roundtrip() {
    let (mut e, mut h) = e_h();
    e.set_vars_text("%one 1\n%two two words\n%empty\n");
    assert_eq!(ev(&mut e, &mut h, "%two"), "two words");
    assert_eq!(e.vars_text(), "%one 1\n%two two words\n%empty\n");
    e.set_vars_text("[variables]\nn0=%ini yes\nn1=%x 2\n");
    assert_eq!(ev(&mut e, &mut h, "%ini %x"), "yes 2");
    assert_eq!(ev(&mut e, &mut h, "%one"), "");
}

#[test]
fn var_identifier() {
    let (mut e, mut h) = e_h();
    run(&mut e, &mut h, "/.set %ab 1 | .set %ac 2 | .set %zz 3");
    assert_eq!(ev(&mut e, &mut h, "$var(%a*,0)"), "2");
    assert_eq!(ev(&mut e, &mut h, "$var(%a*,1)"), "%ab");
    assert_eq!(ev(&mut e, &mut h, "$var(%a*,2).value"), "2");
}

#[test]
fn variable_names_are_case_insensitive() {
    let (mut e, mut h) = e_h();
    run(&mut e, &mut h, "/.set %Name value");
    assert_eq!(ev(&mut e, &mut h, "%name %NAME"), "value value");
}

// ---------------------------------------------------------------------------------------------------------------
// Alias
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn alias_with_params() {
    let (mut e, mut h) = setup("alias greet { echo -a Hello $1- }");
    run(&mut e, &mut h, "/greet bob alice");
    assert_eq!(h.says(), vec!["Hello bob alice"]);
    assert!(e.has_alias("greet"));
    assert!(e.has_alias("/GREET"));
}

#[test]
fn alias_return_as_identifier() {
    let (mut e, mut h) = setup("alias add return $calc($1 + $2)\nalias two { return $1 and $2 }");
    assert_eq!(ev(&mut e, &mut h, "$add(2,3)"), "5");
    assert_eq!(ev(&mut e, &mut h, "$two(a b,c)"), "a b and c");
}

#[test]
fn isid_and_result() {
    let (mut e, mut h) = setup("alias t { if ($isid) return id | echo -a cmd | return 42 }");
    assert_eq!(ev(&mut e, &mut h, "$t"), "id");
    run(&mut e, &mut h, "//t | echo -a res $result");
    assert_eq!(h.says(), vec!["cmd", "res 42"]);
}

#[test]
fn alias_prop() {
    let (mut e, mut h) = setup("alias pp return $prop $+ : $+ $1");
    assert_eq!(ev(&mut e, &mut h, "$pp(x).abc"), "abc:x");
}

#[test]
fn local_aliases() {
    let mut e = engine();
    let mut h = FakeHost::new();
    e.load_text("a.mrc", ScriptKind::Remote, "alias -l secret return A\nalias pubA return $secret").unwrap();
    e.load_text("b.mrc", ScriptKind::Remote, "alias tryit return < $+ $secret $+ >").unwrap();
    assert_eq!(ev(&mut e, &mut h, "$pubA"), "A");
    assert_eq!(ev(&mut e, &mut h, "$tryit"), "<>");
    assert_eq!(ev(&mut e, &mut h, "$secret"), "");
    assert!(!e.has_alias("secret"));
    assert!(e.has_alias("pubA"));
}

#[test]
fn alias_overrides_command_and_bypass() {
    let (mut e, mut h) = setup("alias msg { echo -a intercepted $1- }");
    run(&mut e, &mut h, "/msg #c hi");
    assert_eq!(h.last(), ("echo".into(), "-a intercepted #c hi".into()));
    run(&mut e, &mut h, "/!msg #c hi");
    assert_eq!(h.last(), ("msg".into(), "#c hi".into()));
}

#[test]
fn alias_calling_same_named_command() {
    let (mut e, mut h) = setup("alias join { echo -a joining $1 | join $1 }");
    run(&mut e, &mut h, "/join #x");
    assert_eq!(h.names(), vec!["echo", "join"]);
    assert_eq!(h.last().1, "#x");
}

#[test]
fn aliases_file_kind() {
    let mut e = engine();
    let mut h = FakeHost::new();
    e.load_text("aliases", ScriptKind::Aliases, "/op /mode # +o $$1\nhi echo -a hi $1\nmulti {\n  echo -a m1\n  echo -a m2\n}")
        .unwrap();
    run(&mut e, &mut h, "/hi bob");
    run(&mut e, &mut h, "/multi");
    run(&mut e, &mut h, "/op Carl");
    assert_eq!(h.says(), vec!["hi bob", "m1", "m2"]);
    assert_eq!(h.last(), ("mode".into(), "#test +o Carl".into()));
}

#[test]
fn dynamic_alias_command() {
    let (mut e, mut h) = e_h();
    run(&mut e, &mut h, "/alias foo echo -a dyn $1");
    run(&mut e, &mut h, "/foo x");
    assert_eq!(h.says(), vec!["dyn x"]);
    assert!(e.has_alias("foo"));
    run(&mut e, &mut h, "/alias foo");
    assert!(!e.has_alias("foo"));
}

#[test]
fn isalias_identifier() {
    let (mut e, mut h) = setup("alias greet echo hi");
    assert_eq!(ev(&mut e, &mut h, "$isalias(greet) $isalias(nope)"), "$true $false");
    assert_eq!(ev(&mut e, &mut h, "$isalias(greet).fname"), "test.mrc");
}

#[test]
fn haltdef_in_alias_reaches_event() {
    let (mut e, mut h) = setup("on *:TEXT:*:#:{ stop }\nalias stop haltdef");
    let r = e.fire(&mut h, &ctx(), &alpirc_script::EventInfo {
        kind: Some(alpirc_script::EventKind::Text),
        nick: "Bob".into(),
        chan: Some("#test".into()),
        text: "x".into(),
        ..Default::default()
    });
    assert!(r.haltdef);
}

// ---------------------------------------------------------------------------------------------------------------
// API et erreurs
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn evaluate_api() {
    let (mut e, mut h) = e_h();
    assert_eq!(e.evaluate(&mut h, &Ctx::default(), "$calc(2*21)"), "42");
    assert_eq!(e.evaluate(&mut h, &Ctx::default(), "$gettok(a)"), "");
    assert_eq!(h.errors.len(), 1);
}

#[test]
fn command_api() {
    let (mut e, mut h) = setup("alias hey echo -a hey $1");
    e.command(&mut h, &ctx(), "set", "%cmd $me").unwrap();
    assert_eq!(ev(&mut e, &mut h, "%cmd"), "$me");
    e.command(&mut h, &ctx(), "msg", "#c hi | there").unwrap();
    assert_eq!(h.last(), ("msg".into(), "#c hi | there".into()));
    e.command(&mut h, &ctx(), "hey", "you").unwrap();
    assert_eq!(h.last(), ("echo".into(), "-a hey you".into()));
    e.command(&mut h, &ctx(), "!hey", "you").unwrap();
    assert_eq!(h.last(), ("hey".into(), "you".into()));
    e.command(&mut h, &ctx(), "if", "(1 == 1) echo -a cond").unwrap();
    assert_eq!(h.last(), ("echo".into(), "-a cond".into()));
}

#[test]
fn host_error_propagates() {
    let (mut e, mut h) = e_h();
    let err = run_err(&mut e, &mut h, "/fail");
    assert_eq!(err, "* /fail: insufficient parameters");
}

#[test]
fn script_error_has_location() {
    let (mut e, mut h) = setup("alias t {\n  echo -a ok\n  fail now\n  echo -a never\n}");
    let err = run_err(&mut e, &mut h, "/t");
    assert_eq!(err, "* /fail: insufficient parameters (line 3, test.mrc)");
    assert_eq!(h.says(), vec!["ok"]);
}

#[test]
fn quiet_prefix() {
    let (mut e, mut h) = e_h();
    run(&mut e, &mut h, "/.msg #c secret");
    assert_eq!(h.last(), ("quiet:msg".into(), "#c secret".into()));
}

#[test]
fn command_name_from_identifier() {
    let (mut e, mut h) = e_h();
    run(&mut e, &mut h, "//$iif(1,msg,notice) #c hi");
    assert_eq!(h.last(), ("msg".into(), "#c hi".into()));
}

#[test]
fn sandbox_forbidden_commands() {
    let (mut e, mut h) = e_h();
    let err = run_err(&mut e, &mut h, "/run notepad");
    assert!(err.contains("sandbox"), "{err}");
    let err = run_err(&mut e, &mut h, "//echo $dll(x.dll,f,a)");
    assert!(err.contains("sandbox"), "{err}");
    assert!(h.cmds.is_empty());
}

#[test]
fn noop_evaluates_only() {
    let (mut e, mut h) = setup("alias side { echo -a side }");
    run(&mut e, &mut h, "//noop $side");
    assert_eq!(h.names(), vec!["echo"]);
}

#[test]
fn scid_and_scon() {
    let (mut e, mut h) = e_h();
    run(&mut e, &mut h, "//scid 2 echo -a $!cid");
    assert_eq!(h.cmds.last().unwrap(), &("echo".to_string(), "-a 2".to_string(), Some(2)));
    h.clear();
    run(&mut e, &mut h, "//scon -a echo -a $!cid");
    let got: Vec<(String, Option<u32>)> = h.cmds.iter().map(|(_, a, c)| (a.clone(), *c)).collect();
    assert_eq!(got, vec![("-a 1".to_string(), Some(1)), ("-a 5".to_string(), Some(5))]);
}

#[test]
fn scid_without_command_switches_routine() {
    let (mut e, mut h) = setup("alias t { scid 7 | echo -a $conncheck | scid -r | echo -a $conncheck }");
    run(&mut e, &mut h, "/t");
    assert_eq!(h.says(), vec!["7", "1"]);
}

#[test]
fn run_line_parse_error_free_text() {
    let (mut e, mut h) = e_h();
    // Sans `/` : traité comme du code (évalué).
    run(&mut e, &mut h, "echo -a $me");
    assert_eq!(h.last().1, "-a Guiwald");
}

#[test]
fn load_text_parse_error() {
    let mut e = engine();
    let r = e.load_text("bad.mrc", ScriptKind::Remote, "alias t {\n  echo a\n");
    match r {
        Err(ScriptError::Parse { file, line, .. }) => {
            assert_eq!(file, "bad.mrc");
            assert_eq!(line, 1);
        }
        other => panic!("{other:?}"),
    }
}
