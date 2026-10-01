'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const {summarize,requestSummary}=require('../metrics');
const s=(ns,ticks,id='10:100')=>({monotonic_ns:ns,processes:[{group:'api',identity:id,cpu_ticks:ticks,rss_bytes:10,pss_bytes:5}]});
test('CPU uses one-core percent and actual unequal sample intervals',()=>{const r=summarize([s(0,0),s(5e8,100),s(2e9,300)],100,2);assert.equal(r.groups.api.average_cpu_percent_single_core,150);assert.equal(r.groups.api.sampled_peak_cpu_percent_single_core,200);assert.equal(r.groups.api.cpu_seconds_per_attempt,1.5);});
test('PID reuse does not subtract an unrelated lifetime CPU baseline',()=>{const r=summarize([s(0,1000),s(1e9,10,'10:200'),s(2e9,20,'10:200')],100);assert.equal(r.groups.api.cpu_seconds,.1);assert.equal(r.groups.api.unknown_existing_births,1);});
test('Failed requests remain denominator and zero TTFT is retained',()=>{const r=requestSummary([{ok:true,ttft_ms:0,elapsed_ms:1},{ok:false,ttft_ms:null,elapsed_ms:10}],2);assert.equal(r.attempted,2);assert.equal(r.failed,1);assert.equal(r.ttft_ms.n,1);assert.equal(r.ttft_ms.p99,0);assert.equal(r.success_per_second,.5);});
