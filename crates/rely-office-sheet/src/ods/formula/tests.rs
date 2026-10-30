// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use rely_office_core::excel_formula_from_odf;

use super::*;

#[test]
fn writes_formulas_in_open_formula() {
    for (excel, odf) in [
        ("SUM(A1:B2,1)", "SUM([.A1:.B2];1)"),
        (
            "Uno!$A$3+'Due Fogli'!B1:C2",
            "[$Uno.$A$3]+[$'Due Fogli'.B1:.C2]",
        ),
        ("IF(A1>=1,TRUE,FALSE)", "IF([.A1]>=1;TRUE();FALSE())"),
        ("-A1%^2&\"a\"\"b\"", "-[.A1]%^2&\"a\"\"b\""),
        ("(1+2)*3-4/5", "(1+2)*3-4/5"),
        ("1-(2-3)", "1-(2-3)"),
        ("SUM(A:A)", "SUM([.A1:.A1048576])"),
        (
            "IFS(A1,1)+STDEV.S(B1:B3)",
            "COM.MICROSOFT.IFS([.A1];1)+COM.MICROSOFT.STDEV.S([.B1:.B3])",
        ),
        ("_xlfn.CONCAT(A1,)", "COM.MICROSOFT.CONCAT([.A1];)"),
        ("#N/A", "#N/A"),
        ("A1<>B1", "[.A1]<>[.B1]"),
    ] {
        assert_eq!(odf_formula_text(excel), odf, "{excel}");
    }
    assert_eq!(odf_formula_text("not a formula ("), "not a formula (");
    for excel in [
        "SUM(A1:B2,1)",
        "Uno!$A$3+'Due Fogli'!B1:C2",
        "IF(A1>=1,\"x\",2.5)",
    ] {
        assert_eq!(
            excel_formula_from_odf(&odf_formula_text(excel)),
            excel,
            "back again: {excel}"
        );
    }
}
