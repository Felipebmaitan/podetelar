param([Parameter(Mandatory=$true)][string]$Node,[Parameter(Mandatory=$true)][string]$Output)
$ErrorActionPreference='Stop'
$path=[IO.Path]::GetFullPath($Output);if(Test-Path -LiteralPath $path){throw 'Choose a new probe output.'}
$prefix='FreedomStreamPerformance-'+[guid]::NewGuid().ToString('N');$subscriptions=[Collections.Generic.List[object]]::new();$children=[Collections.Generic.List[Diagnostics.Process]]::new()
$principal=[Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())
$administrator=$principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
try{
 try{
  $subscriptions.Add((Register-CimIndicationEvent -Namespace root/cimv2 -ClassName Win32_ProcessStartTrace -SourceIdentifier "$prefix-start"))
  $subscriptions.Add((Register-CimIndicationEvent -Namespace root/cimv2 -ClassName Win32_ProcessStopTrace -SourceIdentifier "$prefix-stop"))
 }catch{
  @{schema=1;subscriptionSucceeded=$false;administrator=$administrator;errorType=$_.Exception.GetType().FullName;limits='Read-only WMI process-event subscription unavailable in this token. No elevation, policy change or polling/exclusion change was performed.'}|ConvertTo-Json|Set-Content -LiteralPath $path -Encoding utf8
  return
 }
 $started=[Collections.Generic.HashSet[int]]::new();$stopped=[Collections.Generic.HashSet[int]]::new();$owned=[Collections.Generic.HashSet[int]]::new()
 $clock=[Diagnostics.Stopwatch]::StartNew();$cpu=(Get-Process -Id $PID).TotalProcessorTime.TotalMilliseconds
 for($n=0;$n -lt 10;$n++){
  $info=[Diagnostics.ProcessStartInfo]::new((Resolve-Path -LiteralPath $Node).Path);$info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.ArgumentList.Add('-e');$info.ArgumentList.Add('setTimeout(()=>{},100)')
  $child=[Diagnostics.Process]::Start($info);$children.Add($child);$null=$owned.Add($child.Id)
  if(-not $child.WaitForExit(2000)){$child.Kill();throw 'Owned process event fixture timed out.'}
 }
 $deadline=$clock.Elapsed.TotalSeconds+3
 while($clock.Elapsed.TotalSeconds -lt $deadline){
  foreach($kind in @('start','stop')){foreach($event in @(Get-Event -SourceIdentifier "$prefix-$kind" -ErrorAction SilentlyContinue)){
   $observed=[int]$event.SourceEventArgs.NewEvent.ProcessID
   if($owned.Contains($observed)){if($kind -eq 'start'){$null=$started.Add($observed)}else{$null=$stopped.Add($observed)}}
   Remove-Event -EventIdentifier $event.EventIdentifier
  }}
  if($started.Count -eq 10 -and $stopped.Count -eq 10){break}
  [Threading.Thread]::Sleep(25)
 }
 @{schema=1;subscriptionSucceeded=$true;administrator=$administrator;ownedProcesses=10;startsObserved=$started.Count;stopsObserved=$stopped.Count;parentCpuMs=(Get-Process -Id $PID).TotalProcessorTime.TotalMilliseconds-$cpu;wallMs=$clock.Elapsed.TotalMilliseconds;limits='Actual WMI start/stop events for ten owned hidden Node children. No names, paths or other process identities exported. Successful delivery here does not establish loss-free events or identity/exclusion safety; default audio discovery retains its fresh process/creation/session scan.'}|ConvertTo-Json|Set-Content -LiteralPath $path -Encoding utf8
}finally{
 foreach($child in $children){if(-not $child.HasExited){$child.Kill();$child.WaitForExit()};$child.Dispose()}
 foreach($kind in @('start','stop')){Unregister-Event -SourceIdentifier "$prefix-$kind" -ErrorAction SilentlyContinue;Get-Event -SourceIdentifier "$prefix-$kind" -ErrorAction SilentlyContinue|Remove-Event -ErrorAction SilentlyContinue}
 foreach($subscription in $subscriptions){Remove-Job -Id $subscription.Id -Force -ErrorAction SilentlyContinue}
}
