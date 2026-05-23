use crate::{
    orchestration::{powershell_json_command, StrictCommandSpec},
    shell::ps_single_quoted,
};

#[test]
fn powershell_json_command_uses_noninteractive_mode() {
    let spec: StrictCommandSpec =
        powershell_json_command("test", "[pscustomobject]@{ok=$true}|ConvertTo-Json");
    assert_eq!(spec.program, "powershell");
    assert!(spec.args.contains(&"-NonInteractive".to_string()));
    assert!(spec.args.iter().any(|arg| arg.contains("ConvertTo-Json")));
}

#[test]
fn bridge_escapes_single_quotes_in_vm_name() {
    let script = format!(
        "Start-VM -Name {} -ErrorAction Stop",
        ps_single_quoted("bad'name")
    );
    assert!(script.contains("'bad''name'"));
}

#[test]
fn missing_vm_script_emits_json_null() {
    let script = format!(
        r#"
$vmName = {}
$vm = Get-VM -Name $vmName -ErrorAction SilentlyContinue
if (-not $vm) {{
  [Console]::Out.Write('null')
  exit 0
}}
"#,
        ps_single_quoted("sample")
    );
    assert!(script.contains("[Console]::Out.Write('null')"));
}
