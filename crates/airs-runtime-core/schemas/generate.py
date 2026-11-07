import json,sys
def obj(props, req=None):
    return {"type":"object","additionalProperties":False,"properties":props,"required":sorted(req if req is not None else props.keys())}
R=lambda n:{"$ref":f"#/$defs/{n}"}
S=lambda **k:{"type":"string",**k}
I=lambda lo=None,hi=None:{k:v for k,v in (("type","integer"),("minimum",lo),("maximum",hi)) if v is not None}
B={"type":"boolean"}
def arr(x,mx=None,mn=None):
    d={"type":"array","items":x}
    if mx is not None:d["maxItems"]=mx
    if mn is not None:d["minItems"]=mn
    return d
NULL=lambda x:{"anyOf":[x,{"type":"null"}]}
defs={
 "utc":S(pattern=r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d{1,9})?Z$"),
 "range":obj({"since":R("utc"),"until":R("utc")}),
 "cursor":S(pattern=r"^c1\.[A-Za-z0-9_-]{16,2048}$"),
 "page":obj({"cursor":R("cursor"),"limit":I(1,100)},[]),
 "app_ids":arr(S(minLength=1,maxLength=4096),100),
 "schedule_id":S(pattern="^s1_[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$"),
 "job_id":S(pattern=r"^j1_[0-9a-f]{8}_[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$"),
 "preview_id":S(pattern=r"^p1_[0-9a-f]{8}_[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$"),
 "report_id":S(pattern=r"^r1_[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$"),
 "version_id":S(pattern=r"^v1_[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$"),
 "upload_id":S(pattern=r"^u1_[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$"),
 "sha256":S(pattern=r"^[0-9a-f]{64}$"),
 "job_state":{"enum":["queued","running","waiting_retry","needs_credentials","blocked_policy","cancel_requested","canceled","succeeded","partial","failed"]},
 "tier":{"enum":["reader","operator","administrator"]},
 "job":obj({"job_id":R("job_id"),"operation":S(),"state":R("job_state"),"submitted_at":R("utc"),"updated_at":R("utc"),
   "submitter":obj({"kind":{"enum":["principal","service"]},"label":S(maxLength=128)}),
   "params_digest":R("sha256"),"epoch":S(pattern="^[0-9a-f]{8}$"),"progress":NULL(obj({"phase":S(maxLength=64),"done":I(0),"total":NULL(I(0))})),
   "gaps":arr(S(maxLength=256),50),"result":NULL(R("job_result")),"retry_after_seconds":NULL(I(0))}),
 "job_result":{"anyOf":[R("result_collection"),R("result_report"),R("result_enrichment"),R("result_lifecycle"),R("result_export"),R("result_failure")]},
 "result_collection":obj({"operation":{"const":"collection_start"},"sources":arr(R("coverage_source"),100),
   "sends":obj({"oauth":I(0,1000),"management":I(0,1000),"results":I(0,1000),"reports":{"const":0}}),
   "new_facts":I(0),"duplicate_facts":I(0),"unfinished_sources":I(0,100)}),
 "result_report":obj({"operation":{"enum":["reports_create","daily_report","revision","catch_up"]},"report_id":R("report_id"),"version_id":R("version_id"),
   "manifest_sha256":R("sha256"),"completeness":{"enum":["complete","partial"]}}),
 "result_enrichment":obj({"operation":{"const":"enrichment_start"},"selected_ids":I(0,10),"sends":I(0,2),"cached":I(0,10),"stopped_on_429":B}),
 "result_lifecycle":obj({"operation":{"const":"lifecycle_execute"},"action":{"enum":["prune","artifact_delete","compact","backup","restore","recover","import_finalize","allowance_set","epoch_activate"]},
   "affected":obj({"facts":I(0),"receipts":I(0),"examples":I(0),"report_versions":I(0)},[]),"tombstones":I(0),"backup_id":NULL(S(pattern="^b1_[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$")),
   "imported_records":NULL(I(0)),"partial_import":B}),
 "result_export":obj({"operation":{"const":"export"},"bytes":I(0,17179869184),"sha256":R("sha256"),"records":I(0)}),
 "result_failure":obj({"operation":S(pattern="^[a-z_]{1,40}$"),"error":R("error"),"committed_facts":I(0),"uncertain_sends":I(0)}),
 "coverage_source":obj({"source":{"enum":["sessions","results","reports","inventory","profiles"]},"application_id":NULL(S(maxLength=4096)),
   "traversed_as_of":NULL(R("utc")),"state":{"enum":["complete","partial","falling_behind","unavailable","unknown"]},"gap_seconds":NULL(I(0))}),
 "manifest":obj({"report_id":R("report_id"),"version_id":R("version_id"),"version":I(0,24),"predecessor":NULL(R("version_id")),
   "kind":{"enum":["daily","revision","manual","catch_up"]},"period":R("range"),"scheduled_at":NULL(R("utc")),"as_of":R("utc"),
   "timezone":S(),"tzdb_version":S(),"completeness":{"enum":["complete","partial"]},"gaps":arr(S(maxLength=256),100),
   "files":arr(obj({"name":{"enum":["report.html","evidence.json","review.md"]},"bytes":I(0,67108864),"sha256":R("sha256")}),3,3),
   "analysis_version":I(1),"redaction_version":I(1),"job_id":R("job_id")}),
 "receipt":obj({"receipt_key":S(pattern=r"^[A-Za-z0-9_.:-]{8,200}$"),"declared_scanner":S(pattern=r"^[A-Za-z0-9][A-Za-z0-9_-]{0,63}$"),
   "scan_id":S(pattern=r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"),"report_id":NULL(S(maxLength=256)),"session_id":NULL(S(maxLength=4096)),
   "sub_request":NULL(I(0)),"application_id":NULL(S(maxLength=4096)),"application_name":NULL(S(maxLength=4096)),
   "received_at":R("utc"),"category":NULL({"enum":["benign","malicious","unknown"]}),"action":NULL({"enum":["allow","block","unknown"]}),
   "status":{"enum":["complete","pending","failed","timeout"]},"identity_withheld":B},
   ["receipt_key","declared_scanner","scan_id","received_at","status","identity_withheld"]),
 "error":obj({"schema_version":{"const":1},"code":{"enum":["invalid_request","unsupported_version","unauthenticated","forbidden","not_found","conflict","idempotency_conflict","control_restored","overloaded","quota_exhausted","low_space","storage_unavailable","maintenance_required","credential_blocked","upstream_unavailable","canceled","result_too_large","internal"]},
   "reason":S(maxLength=256),"retry_after_seconds":I(0),"job_id":R("job_id"),"credential_status":{"enum":["ok","needs_credentials","cooldown"]}},["schema_version","code","reason"]),
 "accepted":obj({"schema_version":{"const":1},"job":R("job"),"joined_existing":B}),
}
def env(params, idem):
    p={"schema_version":{"const":1},"tenant":S(pattern=r"^[a-z0-9][a-z0-9-]{0,62}$"),"params":params}
    req=["schema_version","tenant","params"]
    if idem:
        p["idempotency_key"]=S(pattern=r"^[A-Za-z0-9_-]{16,64}$"); req.append("idempotency_key")
        # The recovery epoch the client learned from service_info when it first
        # created this key; a different current epoch is `control_restored`.
        p["expected_epoch"]=S(pattern="^[0-9a-f]{8}$"); req.append("expected_epoch")
    return obj(p,req)
def res(props):
    d={"schema_version":{"const":1}}; d.update(props); return obj(d)
paged=lambda item,mx=100: res({"items":arr(item,mx),"next_cursor":NULL(R("cursor"))})
E=obj({},[])
T={}
def tool(name,tier,idem,params,result,limit="agent",mutating=False,admin_proof=False,authority=None):
    if authority is None:
        authority = "direct_proof" if admin_proof else "grant"
    T[name]={"tier":tier,"idempotency_key":idem,"mutating":mutating,"admin_proof":admin_proof,"authority":authority,"result_limit":limit,
             "params":env(params,idem),"result":result}
tool("service_info","reader",False,E,res({"server":S(),"version":S(),"contract_version":I(1),"schema_version_supported":arr(I(1)),
  "negotiated_protocol":NULL(S()),"effective_tier":R("tier"),"tools":arr(S(),64),"health":{"enum":["ok","degraded","maintenance","storage_unavailable"]},"service_tenant":S(),"epoch":S(pattern="^[0-9a-f]{8}$")}))
tool("history_status","reader",False,E,res({"archive_present":B,"schema":NULL(I(1)),"last_collection":NULL(R("job")),"falling_behind":B,"oldest_gap_seconds":NULL(I(0))}))
tool("history_coverage","reader",False,obj({"range":R("range"),"app_ids":R("app_ids"),"page":R("page")},["range"]),paged(R("coverage_source"),100))
tool("history_query","reader",False,obj({"range":R("range"),"app_ids":R("app_ids"),"group_by":arr({"enum":["day","application","detector","verdict","action","profile"]},3),
  "page":R("page")},["range"]),res({"cohort":obj({"events":I(0),"unknown_time":I(0),"unattributed_receipts":I(0)}),
  "rows":arr(obj({"key":obj({},[]) | {"additionalProperties":{"type":["string","null"]}},"events":I(0),"malicious":I(0),"blocked":I(0),"allowed":I(0),"unknown":I(0)}),100),
  "next_cursor":NULL(R("cursor")),"display_truncated":B}))
tool("reports_list","reader",False,obj({"range":R("range"),"kind":{"enum":["daily","revision","manual","catch_up"]},"page":R("page")},[]),paged(R("manifest"),50))
tool("reports_get","reader",False,obj({"report_id":R("report_id"),"version_id":R("version_id")},["report_id"]),res({"manifest":R("manifest"),"summary":S(maxLength=8192),"latest_version":R("version_id")}))
tool("artifacts_read","reader",False,obj({"version_id":R("version_id"),"name":{"enum":["review.md","summary.md"]},"cursor":R("cursor")},["version_id","name"]),
  res({"segment":S(maxLength=6144),"next_cursor":NULL(R("cursor")),"total_bytes":I(0),"sha256":R("sha256")}))
tool("artifacts_download","reader",False,obj({"version_id":R("version_id"),"name":{"enum":["report.html","evidence.json","review.md"]},"offset":I(0,67108863),"length":I(1,65536)}),
  res({"offset":I(0),"length":I(0,65536),"total_bytes":I(0,67108864),"chunk_sha256":R("sha256"),"file_sha256":R("sha256"),"data_base64":S(maxLength=87384)}),limit="cli")
tool("jobs_list","reader",False,obj({"states":arr(R("job_state"),10),"page":R("page")},[]),paged(R("job"),50))
tool("jobs_get","reader",False,obj({"job_id":R("job_id")}),res({"job":R("job")}))
tool("schedules_list","reader",False,E,res({"schedules":arr(obj({"schedule_id":R("schedule_id"),"kind":{"enum":["collection","daily_report","daily_enrichment"]},
  "enabled":B,"interval_minutes":NULL(I(5,60)),"revision_interval_hours":NULL(I(1,24)),"revision_allowance":NULL(I(1,24)),
  "effective_at":R("utc"),"last_completed_slot":NULL(R("utc")),"backlog_slots":I(0)}),16)}))
tool("collection_start","operator",True,obj({"mode":{"enum":["recent","backfill"]},"range":R("range"),"app_ids":R("app_ids")},["mode"]),R("accepted"),mutating=True)
tool("reports_create","operator",True,obj({"range":R("range"),"app_ids":R("app_ids"),"title":S(maxLength=200)},["range"]),R("accepted"),mutating=True)
tool("enrichment_start","operator",True,obj({"report_id":R("report_id"),"max_reports":I(1,10)},["report_id"]),R("accepted"),mutating=True)
tool("jobs_cancel","operator",False,obj({"job_id":R("job_id")}),res({"job":R("job")}),mutating=True)
tool("schedules_set","operator",True,obj({"kind":{"enum":["collection","daily_report","daily_enrichment"]},"enabled":B,"effective_at":R("utc"),
  "interval_minutes":I(5,60),"revision_interval_hours":I(1,24),"revision_allowance":I(1,24)},["kind","enabled","effective_at"]),res({"schedule_id":R("schedule_id"),"revision":I(1)}),mutating=True)
tool("schedules_disable","operator",True,obj({"schedule_id":R("schedule_id"),"effective_at":R("utc")}),res({"schedule_id":R("schedule_id"),"revision":I(1)}),mutating=True)
tool("receipts_ingest","operator",True,obj({"cloud_id":R("sha256"),"records":arr(R("receipt"),200,1)}),
  res({"accepted":I(0,200),"duplicates":I(0,200),"refused":arr(obj({"index":I(0,199),"reason":S(maxLength=128)}),200)}),mutating=True)
tool("assessments_submit","operator",True,obj({"version_id":R("version_id"),"review_sha256":R("sha256"),"markdown":S(maxLength=65536),"reviewer_label":S(maxLength=128)},["version_id","review_sha256","markdown"]),
  res({"assessment_id":S(pattern="^a1_[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$"),"attributed_to":obj({"kind":{"enum":["principal"]},"label":S(maxLength=128)})}),mutating=True)
ACTIONS=["prune","artifact_delete","compact","backup","restore","recover","import_finalize","export","allowance_set","epoch_activate","upload_admit"]
tool("lifecycle_preview","administrator",False,obj({"action":{"enum":ACTIONS},"before":R("utc"),"app_ids":R("app_ids"),"examples_only":B,
  "artifact_version_ids":arr(R("version_id"),100),"backup_id":S(maxLength=64),"upload_id":R("upload_id"),
  "allowance":obj({"daily_sends":I(0,10),"daily_ids":I(0,50)}),
  "upload":obj({"kind":{"enum":["logical_transfer","legacy_bundle"]},"total_bytes":I(1,17179869184),"sha256":R("sha256"),
    "expect_tsg":S(pattern=r"^[0-9]{1,32}$"),"expect_environment":S(pattern=r"^env1:[0-9a-f]{64}$")})},["action"]),
  res({"preview_id":R("preview_id"),"action":{"enum":ACTIONS},"parameter_digest":R("sha256"),"data_generation":I(0),"expires_at":R("utc"),
  "proof_nonce":NULL(S(pattern="^[0-9a-f]{32}$")),"summary":S(maxLength=8192),"affected":obj({"facts":I(0),"receipts":I(0),"examples":I(0),"report_versions":I(0),"known_backups":I(0)},[])}))
tool("lifecycle_execute","administrator",True,obj({"preview_id":R("preview_id"),"parameter_digest":R("sha256"),"approval_id":S(pattern="^q1_[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$")},["preview_id","parameter_digest"]),R("accepted"),limit="agent",mutating=True,admin_proof=True,authority="direct_proof_or_approval")
tool("admin_confirm","administrator",False,obj({"preview_id":R("preview_id"),"parameter_digest":R("sha256")}),res({"approval_id":S(pattern="^q1_[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$"),"expires_at":R("utc")}),mutating=True,admin_proof=True)
tool("transfer_upload_begin","administrator",True,obj({"preview_id":R("preview_id"),"parameter_digest":R("sha256")}),
  res({"upload_id":R("upload_id"),"chunk_bytes":I(1,65536),"total_bytes":I(1,17179869184),"sha256":R("sha256"),"expires_at":R("utc")}),mutating=True,admin_proof=True)
tool("transfer_upload_chunk","administrator",False,obj({"upload_id":R("upload_id"),"offset":I(0),"data_base64":S(maxLength=87384),"chunk_sha256":R("sha256")}),
  res({"received_bytes":I(0)}),mutating=True,admin_proof=False,authority="upload_session")
tool("transfer_upload_finalize","administrator",False,obj({"upload_id":R("upload_id")}),res({"upload_id":R("upload_id"),"validated":B,"records":I(0)}),mutating=True,admin_proof=False,authority="upload_session")
tool("transfer_upload_abort","administrator",False,obj({"upload_id":R("upload_id")}),res({"upload_id":R("upload_id"),"aborted":B}),mutating=True,admin_proof=False,authority="upload_session")
tool("export_read","administrator",False,obj({"job_id":R("job_id"),"offset":I(0),"length":I(1,65536)}),
  res({"offset":I(0),"length":I(0,65536),"total_bytes":I(0),"chunk_sha256":R("sha256"),"file_sha256":R("sha256"),"data_base64":S(maxLength=87384)}),limit="cli")
doc={"$comment":"Frozen M0 tool catalogue (docs/RUNTIME_MCP_CONTRACT.md §4). Changing it after M0 acceptance is a contract revision.",
 "format":"airs-mcp-tools","schema_version":1,"result_limits":{"agent":16384,"cli":262144},
 "authority_rules":{
   "grant":"Effective tier (token scope ∩ grant) ≥ tool tier and the tool is in the grant's allowed list. A mutating 'grant' tool is refused for an Administrator grant in every mode.",
   "direct_proof":"Administrator grant, admin.mode=fresh_proof, and an assertion carrying the preview's server nonce (proof_nonce_claim) with fresh auth_time; the proof is consumed once together with the effect. Refused when admin.mode=disabled.",
   "direct_proof_or_approval":"Either direct_proof by the administrative identity that will own the job, or an unexpired approval_id issued by admin_confirm for exactly this preview, digest and data generation and presented by the same agent actor that created the preview; the approval is consumed with the job insert. A proof or approval is never reusable for another preview or operation.",
   "upload_session":"Only the actor whose transfer_upload_begin admitted the session, within its expiry, with Administrator effective tier; no new proof per chunk (continuation of one admitted operation); offsets must be contiguous and never exceed the declared length; finalize requires the complete length and matching SHA-256 and is idempotent; abort by that actor only and not after import. The import itself is a separate import_finalize preview (owner only, finalized uploads only, once per upload) requiring its own direct proof.",
   "idempotent_retry":"A retry with the same idempotency key returns the original result only if every admitted binding field (preview_id, parameter_digest, data generation, and approval_id where used) is identical; otherwise idempotency_conflict."},
 "$defs":defs,"tools":T}
json.dump(doc,open(sys.argv[1],'w'),indent=1,sort_keys=True)
print(len(T))

# ---- examples (fixtures): one valid request and result per tool ----
U="00000000-0000-4000-8000-000000000001"
H="a"*64
JOB=f"j1_0000000a_{U}"; PRE=f"p1_0000000a_{U}"; REP=f"r1_{U}"; VER=f"v1_{U}"; UP=f"u1_{U}"
T0="2026-11-03T06:00:00Z"; T1="2026-11-04T06:00:00Z"
CUR="c1."+"A"*24
job={"job_id":JOB,"operation":"collection_start","state":"queued","submitted_at":T1,"updated_at":T1,
     "submitter":{"kind":"principal","label":"claude-code"},"params_digest":H,"epoch":"0000000a","progress":None,"gaps":[],"result":None,"retry_after_seconds":None}
man={"report_id":REP,"version_id":VER,"version":0,"predecessor":None,"kind":"daily","period":{"since":T0,"until":T1},"scheduled_at":T1,"as_of":T1,
     "timezone":"America/Chicago","tzdb_version":"jiff-tzdb 0.1.9","completeness":"partial","gaps":["reports enrichment disabled"],
     "files":[{"name":n,"bytes":10,"sha256":H} for n in ("report.html","evidence.json","review.md")],"analysis_version":1,"redaction_version":1,"job_id":JOB}
def acc_for(op):
    return {"schema_version":1,"job":dict(job,operation=op),"joined_existing":False}
acc=acc_for("collection_start")
rec={"receipt_key":"bulk:job1:item:0001","declared_scanner":"monitor","scan_id":U,"report_id":None,"session_id":None,"sub_request":None,
     "application_id":None,"application_name":None,"received_at":T1,"category":"malicious","action":"block","status":"complete","identity_withheld":False}
ex={
 "service_info":({},{"server":"airs-mcp","version":"8.2.0","contract_version":1,"schema_version_supported":[1],"negotiated_protocol":"2025-06-18","effective_tier":"reader","tools":["service_info"],"health":"ok","service_tenant":"tsg-1","epoch":"0000000a"}),
 "history_status":({},{"archive_present":True,"schema":1,"last_collection":job,"falling_behind":False,"oldest_gap_seconds":None}),
 "history_coverage":({"range":{"since":T0,"until":T1}},{"items":[{"source":"sessions","application_id":"app-a","traversed_as_of":T1,"state":"complete","gap_seconds":None}],"next_cursor":None}),
 "history_query":({"range":{"since":T0,"until":T1},"group_by":["day"],"page":{"limit":50}},{"cohort":{"events":10,"unknown_time":0,"unattributed_receipts":1},"rows":[{"key":{"day":"2026-11-03"},"events":10,"malicious":2,"blocked":1,"allowed":1,"unknown":0}],"next_cursor":CUR,"display_truncated":False}),
 "reports_list":({},{"items":[man],"next_cursor":None}),
 "reports_get":({"report_id":REP},{"manifest":man,"summary":"Daily brief (partial).","latest_version":VER}),
 "artifacts_read":({"version_id":VER,"name":"review.md"},{"segment":"# Review\n","next_cursor":None,"total_bytes":9,"sha256":H}),
 "artifacts_download":({"version_id":VER,"name":"report.html","offset":0,"length":65536},{"offset":0,"length":3,"total_bytes":3,"chunk_sha256":H,"file_sha256":H,"data_base64":"PGg+"}),
 "jobs_list":({"states":["queued"]},{"items":[job],"next_cursor":None}),
 "jobs_get":({"job_id":JOB},{"job":job}),
 "schedules_list":({},{"schedules":[{"schedule_id":f"s1_{U}","kind":"daily_report","enabled":True,"interval_minutes":None,"revision_interval_hours":4,"revision_allowance":6,"effective_at":T1,"last_completed_slot":T1,"backlog_slots":0}]}),
 "collection_start":({"mode":"recent"},acc),
 "reports_create":({"range":{"since":T0,"until":T1}},acc_for("reports_create")),
 "enrichment_start":({"report_id":REP,"max_reports":5},acc_for("enrichment_start")),
 "jobs_cancel":({"job_id":JOB},{"job":dict(job,state="cancel_requested")}),
 "schedules_set":({"kind":"collection","enabled":True,"effective_at":T1,"interval_minutes":15},{"schedule_id":f"s1_{U}","revision":2}),
 "schedules_disable":({"schedule_id":f"s1_{U}","effective_at":T1},{"schedule_id":f"s1_{U}","revision":3}),
 "receipts_ingest":({"cloud_id":H,"records":[rec]},{"accepted":1,"duplicates":0,"refused":[]}),
 "assessments_submit":({"version_id":VER,"review_sha256":H,"markdown":"No action needed."},{"assessment_id":f"a1_{U}","attributed_to":{"kind":"principal","label":"claude-code"}}),
 "lifecycle_preview":({"action":"prune","before":T0},{"preview_id":PRE,"action":"prune","parameter_digest":H,"data_generation":7,"expires_at":T1,"proof_nonce":"0"*32,"summary":"Prune facts before 2026-11-03.","affected":{"facts":5,"report_versions":0,"known_backups":1}}),
 "lifecycle_execute":({"preview_id":PRE,"parameter_digest":H},acc_for("lifecycle_execute")),
 "admin_confirm":({"preview_id":PRE,"parameter_digest":H},{"approval_id":f"q1_{U}","expires_at":T1}),
 "transfer_upload_begin":({"preview_id":PRE,"parameter_digest":H},{"upload_id":UP,"chunk_bytes":65536,"total_bytes":100,"sha256":H,"expires_at":T1}),
 "transfer_upload_chunk":({"upload_id":UP,"offset":0,"data_base64":"AAAA","chunk_sha256":H},{"received_bytes":3}),
 "transfer_upload_finalize":({"upload_id":UP},{"upload_id":UP,"validated":True,"records":10}),
 "transfer_upload_abort":({"upload_id":UP},{"upload_id":UP,"aborted":True}),
 "export_read":({"job_id":JOB,"offset":0,"length":65536},{"offset":0,"length":3,"total_bytes":3,"chunk_sha256":H,"file_sha256":H,"data_base64":"AAAA"}),
}
assert set(ex)==set(T), set(T)^set(ex)
out={}
for n,(p,r) in ex.items():
    req={"schema_version":1,"tenant":"tsg-1","params":p}
    if T[n]["idempotency_key"]:
        req["idempotency_key"]="0123456789abcdef0123"; req["expected_epoch"]="0000000a"
    res_v=r if "schema_version" in r else dict({"schema_version":1},**r)
    out[n]={"request":req,"result":res_v}
ok_collection=dict(job,state="succeeded",result={"operation":"collection_start","sources":[{"source":"sessions","application_id":"app-a","traversed_as_of":T1,"state":"complete","gap_seconds":None}],
  "sends":{"oauth":1,"management":4,"results":2,"reports":0},"new_facts":12,"duplicate_facts":3,"unfinished_sources":0})
partial_report=dict(job,operation="reports_create",state="partial",gaps=["sessions falling behind"],result={"operation":"reports_create","report_id":REP,"version_id":VER,"manifest_sha256":H,"completeness":"partial"})
failed=dict(job,state="failed",result={"operation":"collection_start","error":{"schema_version":1,"code":"credential_blocked","reason":"management credential denied","credential_status":"needs_credentials"},"committed_facts":5,"uncertain_sends":1})
upload_preview=({"action":"upload_admit","upload":{"kind":"logical_transfer","total_bytes":100,"sha256":H,"expect_tsg":"1234567890","expect_environment":"env1:"+H}})
json.dump({"format":"airs-mcp-tool-examples","schema_version":1,"examples":out,
  "job_examples":{"succeeded_collection":ok_collection,"partial_report":partial_report,"failed_collection":failed},
  "extra_requests":{"lifecycle_preview":[{"schema_version":1,"tenant":"tsg-1","params":upload_preview}]}},open(sys.argv[1].replace("tools.v1.json","examples.v1.json"),'w'),indent=1,sort_keys=True)
print("examples",len(out))
