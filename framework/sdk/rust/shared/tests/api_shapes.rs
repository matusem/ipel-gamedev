use upjs_gdd_shared_types::ValidationReport;

#[test]
fn report_shape_matches_expected_fields() {
    let input = r#"{
        "ok": true,
        "errors": 0,
        "warnings": 1,
        "infos": 2,
        "diagnostics": [{"severity":"info","code":"C1","message":"ok"}]
    }"#;
    let report: ValidationReport = serde_json::from_str(input).expect("valid report");
    assert!(report.ok);
    assert_eq!(report.warnings, 1);
    assert_eq!(report.diagnostics.len(), 1);
}
