// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use super::filter_xml;
use crate::address::CellRef;
use crate::model::Workbook;
use crate::value::{ErrorKind, Locale, Value};

const FEED: &str = r#"<?xml version="1.0"?>
<rss version="2.0"><channel><title>Notizie &amp; avvisi</title>
<item id="a"><title>Primo</title><views>12</views></item>
<item id="b"><title>Secondo</title><views>7.5</views></item>
<item id="c"><title><![CDATA[Terzo <b>]]></title></item>
</channel></rss>"#;

fn texts(path: &str) -> Vec<Value> {
    filter_xml(FEED, path)
        .unwrap()
        .into_iter()
        .map(|mut row| row.remove(0))
        .collect()
}

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

#[test]
fn selects_by_the_abbreviated_xpath() {
    assert_eq!(
        texts("//item/title"),
        vec![text("Primo"), text("Secondo"), text("Terzo <b>")]
    );
    assert_eq!(texts("/rss/channel/title"), vec![text("Notizie & avvisi")]);
    assert_eq!(
        texts("rss/channel/title/text()"),
        vec![text("Notizie & avvisi")]
    );
    assert_eq!(texts("//item/@id"), vec![text("a"), text("b"), text("c")]);
    assert_eq!(texts("//item[2]/title"), vec![text("Secondo")]);
    assert_eq!(texts("//item[last()]/@id"), vec![text("c")]);
    assert_eq!(
        texts("//views"),
        vec![Value::Number(12.0), Value::Number(7.5)]
    );
    assert_eq!(texts("/rss/@version"), vec![Value::Number(2.0)]);
    assert_eq!(texts("//channel/*[1]"), vec![text("Notizie & avvisi")]);
    assert_eq!(texts("//views/../@id"), vec![text("a"), text("b")]);
    assert_eq!(texts("//item[1]/./title"), vec![text("Primo")]);
    assert_eq!(texts("//item[1]/@*"), vec![text("a")]);
    assert_eq!(texts("//item[3]/title/node()"), vec![text("Terzo <b>")]);
}

#[test]
fn refuses_what_it_does_not_read() {
    for path in [
        "",
        "/",
        "//item[0]",
        "//item[@id='a']",
        "count(//item)",
        "//item/",
        "a b",
    ] {
        assert_eq!(filter_xml(FEED, path), Err(ErrorKind::Value), "{path}");
    }
    assert_eq!(filter_xml(FEED, "//missing"), Err(ErrorKind::Value));
    for xml in [
        "<a><b></a>",
        "<a/><b/>",
        "text",
        "",
        "<!DOCTYPE a [<!ENTITY x \"y\">]><a>&x;</a>",
        "<a>&x;</a>",
        "<a>x</a>trailing",
    ] {
        assert_eq!(filter_xml(xml, "/a"), Err(ErrorKind::Value), "{xml}");
    }
    assert_eq!(
        filter_xml("<a>&#x41;&lt;</a>", "/a"),
        Ok(vec![vec![text("A<")]])
    );
}

#[test]
fn spills_what_it_selects_down_a_column() {
    let mut workbook = Workbook::new();
    let source = CellRef::parse("A1").unwrap();
    workbook
        .set_input(
            0,
            source,
            "<r><v>1</v><v>2</v><v>3</v></r>",
            Locale::English,
        )
        .unwrap();
    let target = CellRef::parse("B1").unwrap();
    workbook
        .set_input(0, target, "=SUM(FILTERXML(A1,\"//v\"))", Locale::English)
        .unwrap();
    assert_eq!(workbook.value(0, target), Value::Number(6.0));
    workbook
        .set_input(0, target, "=FILTERXML(A1,\"//v\")", Locale::English)
        .unwrap();
    assert_eq!(
        workbook.value(0, CellRef::parse("B3").unwrap()),
        Value::Number(3.0)
    );
    workbook
        .set_input(0, target, "=FILTERXML(A1)", Locale::English)
        .unwrap();
    assert_eq!(workbook.value(0, target), Value::Error(ErrorKind::Value));
}
