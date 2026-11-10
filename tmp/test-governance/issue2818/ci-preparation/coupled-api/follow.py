#!/usr/bin/env python3
"""W5 cost owner: parallel bounded stdout/small ZIP, preserve actual old/current attribution."""
import hashlib,json,os,re,statistics,subprocess,sys,time,urllib.request,urllib.error,zipfile
from pathlib import Path
from datetime import datetime,timezone
from concurrent.futures import ThreadPoolExecutor
SOURCE=None
def now():return datetime.now(timezone.utc).isoformat()
def save(p,v):p.write_text(json.dumps(v,indent=2,sort_keys=True)+'\n')
def read(p):return json.loads(p.read_text())

def main(run_id,out_dir,control):
    global SOURCE
    bound=json.loads(subprocess.check_output(['git','show',control+':tmp/test-governance/issue2818/ci-preparation/coupled-api/binding.json'],text=True,timeout=10));SOURCE=bound['candidate'];assert isinstance(SOURCE,str) and re.fullmatch(r'[0-9a-f]{40}',SOURCE)
    out=Path(out_dir).resolve();out.mkdir(parents=True,exist_ok=True);start=time.monotonic()
    with (out/'watch.log').open('wb') as f:w=subprocess.run(['gh','run','watch',str(run_id),'--repo','taichuy/1flowbase','--interval','20','--exit-status'],stdout=f,stderr=subprocess.STDOUT,timeout=6000)
    watched={'exit':w.returncode,'elapsed_seconds':time.monotonic()-start,'finished_at':now()}
    run=json.loads(subprocess.check_output(['gh','run','view',str(run_id),'--repo','taichuy/1flowbase','--json','databaseId,status,conclusion,headSha,url,attempt,jobs'],text=True,timeout=45));save(out/'run-final.json',run);assert run['status']=='completed' and run['headSha']==control
    class NoRedirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self,req,fp,code,msg,headers,newurl):return None
    def signed(url):
        token=subprocess.check_output(['gh','auth','token'],text=True).strip()
        try:urllib.request.build_opener(NoRedirect).open(urllib.request.Request(url,headers={'Authorization':'Bearer '+token}),timeout=25);raise RuntimeError('expected signed redirect')
        except urllib.error.HTTPError as e:assert e.code==302;return e.headers['Location']
        finally:token=None
    def cause():
        began=time.monotonic();v={'run_id':run_id,'source_sha':SOURCE,'control_sha':control,'byte_cap':262144,'started_at':now()}
        try:
            with urllib.request.urlopen(signed('https://api.github.com/repos/taichuy/1flowbase/actions/jobs/'+str(run['jobs'][0]['databaseId'])+'/logs'),timeout=30) as r:raw=r.read(v['byte_cap']+1)
            v.update(bytes_read=min(len(raw),v['byte_cap']),truncated=len(raw)>v['byte_cap']);phases=[];errors=[];lines=[]
            for line in raw[:v['byte_cap']].decode(errors='replace').splitlines():
                if 'ci_phase_summary' in line:
                    try:row=json.loads(line[line.index('{'):])['ci_phase_summary']
                    except (ValueError,KeyError):continue
                    assert row['source_sha'] in [SOURCE,'9f35e154b6908eb54b51d1ceb504552b0a4e0eba'];phases.append(row)
                if any(x in line for x in ['RuntimeError','AssertionError','error:','panicked']):errors.append(line[:1500])
                if any(x in line for x in ['validation=','sample\t','heap\t','source='+SOURCE]):lines.append(line[:2000])
            v.update(phases=phases,first_failure=next((r for r in phases if r['exit']!=0 or r['classification']!='exit'),None),errors=errors[-8:],raw_host_lines=lines)
        except Exception as e:v['transport_error']=type(e).__name__+': '+str(getattr(e,'reason',e))[:300]
        v.update(elapsed_seconds=time.monotonic()-began,ready_at=now());save(out/'stdout-phase-attribution.json',v);print(json.dumps({'stdout_first_attribution':v}),flush=True);return v
    def small():
        began=time.monotonic();v={'run_id':run_id,'exit':1,'started_at':now(),'transport':'ordinary whole small ZIP via7897/TLS; no Range/cache'}
        try:
            assert os.environ.get('HTTPS_PROXY')=='http://127.0.0.1:7897'
            meta=json.loads(subprocess.check_output(['gh','api','repos/taichuy/1flowbase/actions/runs/'+str(run_id)+'/artifacts'],text=True,timeout=45));save(out/'artifact-meta.json',meta)
            name='issue2818-coupled-api-results-'+str(run_id)+'-'+str(run['attempt']);a=next(x for x in meta['artifacts'] if x['name']==name);assert not a['expired'] and a['size_in_bytes']<=64_000_000 and a['workflow_run']['head_sha']==control
            v['artifact']=a;archive=out/'small-results.zip';received=0
            with urllib.request.urlopen(signed(a['archive_download_url']),timeout=25) as r,archive.open('wb') as f:
                length=int(r.headers.get('Content-Length','0'));assert not length or length<=64_000_000
                while True:
                    assert time.monotonic()-began<180
                    chunk=r.read(65536)
                    if not chunk:break
                    received+=len(chunk);assert received<=64_000_000;f.write(chunk)
            digest=hashlib.sha256(archive.read_bytes()).hexdigest();assert received==a['size_in_bytes'];assert not a.get('digest') or a['digest']=='sha256:'+digest
            with zipfile.ZipFile(archive) as z:
                assert sum(x.file_size for x in z.infolist())<=1_000_000_000
                for x in z.infolist():assert Path(x.filename).parts and '..' not in Path(x.filename).parts and not x.filename.startswith('/') and 'seed-target-and-source.tar.zst' not in x.filename and 'test-binaries/' not in x.filename
                z.extractall(out/'results')
            v.update(exit=0,bytes=received,zip_sha256=digest,other_cloud_artifacts_not_downloaded=[x for x in meta['artifacts'] if x['name']!=name])
        except Exception as e:v['transport_error']=type(e).__name__+': '+str(getattr(e,'reason',e))[:300]
        v.update(elapsed_seconds=time.monotonic()-began,ready_at=now());save(out/'small-results-transport.json',v);return v
    with ThreadPoolExecutor(max_workers=2) as pool:
        first=pool.submit(cause);light=pool.submit(small);c=first.result();s=light.result()
    assert s['exit']==0,'small transport failed; stdout preserved; no Rust retry'
    dirs=list((out/'results').rglob('ci-evidence'));assert len(dirs)==1;e=dirs[0];optional=lambda n,default:read(e/n) if (e/n).exists() else default
    phases=[read(x) for x in sorted(e.glob('*.summary.json'))];state=optional('qualification-phase-state.json',{});failure=optional('failure.json',None);budget=optional('budget-after.json',{});cost=optional('cost-summary.json',{});A=optional('A-qualified.json',None);ownership=optional('actual-codec-mono-ownership.json',None)
    errors=[]
    if failure:
        cmd=next((x for x in cost.get('commands',[]) if x['label']==failure['phase']),None)
        if cmd:
            log=e/cmd['merged_log'].split('/ci-evidence/',1)[1]
            for line in log.read_text(errors='replace').splitlines():
                try:row=json.loads(line)
                except ValueError:continue
                if row.get('reason')=='compiler-message' and row['message'].get('level')=='error':errors.append({'target':row['target']['name'],'message':row['message']['message'],'rendered':(row['message'].get('rendered') or '')[:3000],'primary_spans':[x for x in row['message'].get('spans',[]) if x.get('is_primary')]})
    actual={name:optional(name,None) for name in ['owner-guards-actual.json','original24-actual.json','native7-actual.json','consumer-0-actual.json','consumer-1-actual.json','consumer-2-actual.json','consumer-3-actual.json','dependency-boundaries-actual.json','C01-old-literal-detection.json','C01-new-alltargets-detection.json']}
    quick={'run_id':run_id,'candidate_sha':SOURCE,'control_sha':control,'CI_conclusion':run['conclusion'],'state':state,'failure':failure,'compiler_errors':errors[:4],'A':None if A is None else {'Fresh':A['Fresh'],'rebuilt':A['rebuilt'],'API_features':A['API_artifact']['features'],'API_profile':A['API_artifact']['profile'],'owner_features':A['owner_artifact']['features']},'mono_ownership':None if ownership is None else {'all_nine_concrete_codecs_owned':ownership['all_nine_concrete_codecs_owned'],'rows':[{'dto':x['dto'],'codec':x['codec'],'owner_rows':len(x['owner_rows']),'API_codec_rows':len(x['API_codec_rows']),'API_DTO_residual_rows':len(x['API_DTO_residual_rows'])} for x in ownership['rows']],'limit':ownership['limit']},'actual_evidence':{k:None if v is None else (v if k.startswith('C01') else {'count':len(v.get('actual',[]))}) for k,v in actual.items()},'first_failed_phase':next((x for x in phases if x['exit']!=0 or x['classification']!='exit'),None),'all_cleanup_complete':all(x['cleanup_complete'] for x in phases),'final_populated_zero':'populated 0' in (e/'final-cgroup.events').read_text() if (e/'final-cgroup.events').exists() else False,'job_seconds':sum((datetime.fromisoformat(x['completedAt'].replace('Z','+00:00'))-datetime.fromisoformat(x['startedAt'].replace('Z','+00:00'))).total_seconds() for x in run['jobs']),'all_command_wall_seconds':cost.get('all_command_wall_seconds'),'budget_after':budget,'cache_retention':optional('seed-retention.json',None),'not_Runtime_RSS_or_Root_PASS':True}
    save(out/'minimal-result.json',quick);print(json.dumps({'minimal_raw_result':quick}),flush=True)
    result={'status':'COUPLED_WORKLOAD_COMPLETE' if state.get('phase')=='complete' else 'COUPLED_FAIL_OR_INCOMPLETE','source_sha':SOURCE,'control_sha':control,'run_id':run_id,'CI_conclusion':run['conclusion'],'minimal_result':quick,'phases':phases,'watch':watched,'stdout_first_attribution':c,'small_results_transport':s,'small_raw_receipt_ready_at':now(),'cache_not_downloaded_locally':True}
    save(out/'attribution-receipt.json',result);print(json.dumps({'coupled_raw_complete':{'status':result['status'],'source_sha':SOURCE,'run_id':run_id,'failure':failure},'receipt':str(out/'attribution-receipt.json')}),flush=True)
if __name__=='__main__':main(int(sys.argv[1]),sys.argv[2],sys.argv[3])
