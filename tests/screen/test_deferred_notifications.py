"""Bounded protocol and Screen integration tests. All app responses are simulated.

Only private sockets/processes are touched; never forwards a fixture to the app.
"""
import copy
import importlib.util
import json
import os
from pathlib import Path
import signal
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
sys.dont_write_bytecode = True
import screen_chat_notify as notify


def state(busy=()):
    return {'turns':[{'status':'inProgress','items':[dict(id=name,type='commandExecution',status='inProgress',completedAtMs=None) for name in busy]}], 'requests':[]}


class FakeApp:
    def __init__(self, state_fn=None, fault=None, delayed_reply=0, owner_switch=None, fragmented=False, response_filter=None):
        self.temp = tempfile.TemporaryDirectory(prefix='ipc-fixture-',dir='/tmp')
        self.path = str(Path(self.temp.name)/'ipc.sock')
        self.server = socket.socket(socket.AF_UNIX)
        self.server.bind(self.path)
        self.server.listen()
        self.server.settimeout(.05)
        self.start = time.monotonic()
        self.state_fn = state_fn or (lambda elapsed: state())
        self.fault = fault
        self.delayed_reply = delayed_reply
        self.owner_switch = owner_switch
        self.fragmented = fragmented
        self.response_filter = response_filter
        self.stop = threading.Event()
        self.sent = threading.Event()
        self.clients = []
        self.workers = []
        self.lock = threading.Lock()
        self.messages = []
        self.requests = []
        self.subscriptions = 0
        self.unsubscriptions = 0
        self.owner_queries = 0
        self.threads = [threading.Thread(target=self.accept),threading.Thread(target=self.tick)]
        for thread in self.threads:thread.start()

    def owner(self):
        return 'owner-2' if self.owner_switch is not None and time.monotonic()-self.start>=self.owner_switch else 'owner-1'

    def send(self, client, data):
        raw=json.dumps(data).encode();frame=struct.pack('<I',len(raw))+raw
        with client['lock']:
            if self.fragmented:
                for part in (frame[:2],frame[2:9],frame[9:]):client['socket'].sendall(part)
            else:client['socket'].sendall(frame)

    def accept(self):
        while not self.stop.is_set():
            try:sock,_=self.server.accept()
            except (socket.timeout,OSError):continue
            sock.settimeout(.05)
            client={'socket':sock,'lock':threading.Lock(),'following':False,'revision':0}
            with self.lock:self.clients.append(client)
            worker=threading.Thread(target=self.client,args=(client,));self.workers.append(worker);worker.start()

    def client(self, client):
        buffer=bytearray()
        try:
            while not self.stop.is_set():
                try:chunk=client['socket'].recv(65536)
                except socket.timeout:continue
                if not chunk:return
                buffer.extend(chunk)
                while len(buffer)>=4:
                    length=struct.unpack('<I',buffer[:4])[0]
                    if len(buffer)<4+length:break
                    message=json.loads(buffer[4:4+length]);del buffer[:4+length]
                    method=message.get('method')
                    if message.get('type')=='broadcast':
                        if method=='thread-stream-following-changed':
                            client['following']=message['params']['following']
                            if client['following']:
                                with self.lock:self.subscriptions+=1;client['subscription']=self.subscriptions
                                client['owner_at_subscribe']=self.owner()
                                if self.fault=='disconnect_once' and client['subscription']==1:
                                    client['socket'].close();return
                            else:self.unsubscriptions+=1
                        continue
                    if message.get('type')!='request':continue
                    self.requests.append(method)
                    response={'type':'response','requestId':message['requestId'],'resultType':'success','result':{}}
                    if method=='initialize':response['result']={'clientId':'fixture-'+str(id(client))}
                    elif method=='thread-owner-discovery':
                        self.owner_queries+=1
                        if self.fault=='no_owner_reply':continue
                        response.update(handledByClientId=self.owner())
                        if self.fault=='owner_rejected':response.update(resultType='error',error='no-client-found')
                    elif method in ('thread-follower-steer-turn','thread-follower-start-turn'):
                        payload=message['params']
                        text=(payload.get('input') or payload.get('turnStart',{}).get('request',{}).get('input'))[0]['text']
                        self.messages.append({'at':time.monotonic(),'method':method,'text':text,'target':message.get('targetClientId')})
                        self.sent.set()
                        if self.fault=='drop_after_send':client['socket'].close();return
                        if self.delayed_reply:self.stop.wait(self.delayed_reply)
                        if self.fault in ('inactive','invalid_start_ack') and method=='thread-follower-steer-turn':
                            response.update(resultType='error',error='Cannot steer conversation fake-thread because its active turn already ended')
                        elif self.fault=='rejected':response.update(resultType='error',error='permission denied')
                        elif self.fault=='bad_ack':response['result']={}
                        elif self.fault=='nonempty_bad_ack':response['result']={'result':{'unrelated':True}}
                        elif self.fault=='empty_turn_id':response['result']={'result':{'turnId':''}}
                        elif self.fault=='invalid_start_ack':response['result']={'result':{'turnId':'wrong-start-shape'}}
                        elif self.fault=='no_send_reply':continue
                        else:response['result']={'result':{'turnId':'fixture-turn'} if method=='thread-follower-steer-turn' else {'turn':{'id':'fixture-turn'}}}
                    if self.response_filter:
                        response=self.response_filter(method,response)
                    self.send(client,response)
        except (OSError,ValueError):pass
        finally:
            client['following']=False
            try:client['socket'].close()
            except OSError:pass

    def tick(self):
        while not self.stop.wait(.02):
            with self.lock:clients=list(self.clients)
            for client in clients:
                if not client['following'] or self.fault=='no_snapshot':continue
                if self.fault=='silent_owner_change' and client.get('owner_at_subscribe')!=self.owner():continue
                try:
                    client['revision']+=1
                    revision=client['revision']
                    change={'type':'snapshot','revision':revision,'conversationState':self.state_fn(time.monotonic()-self.start)}
                    version=99 if self.fault=='version' else 13
                    if self.fault=='regress_once' and client.get('subscription')==1:
                        change['revision']=10 if revision==1 else 1
                    if self.fault=='burst' and not client.get('burst_sent') and time.monotonic()-self.start>.09:
                        client['burst_sent']=time.monotonic()
                        change['conversationState']=state(['brief-tool'])
                        self.send(client,dict(type='broadcast',method='thread-stream-state-changed',version=13,sourceClientId=self.owner(),params={'hostId':'local','conversationId':'fake-thread','change':change}))
                        client['revision']+=1
                        change={'type':'snapshot','revision':client['revision'],'conversationState':state()}
                    if self.fault=='gap_once' and client.get('subscription')==1 and revision>1:
                        change={'type':'patches','revision':revision,'baseRevision':999,'patches':[]}
                    if self.fault=='boolean_base_once' and client.get('subscription')==1 and revision>1:
                        change={'type':'patches','revision':revision,'baseRevision':True,'patches':[]}
                    if self.fault=='invalid_patch_once' and client.get('subscription')==1 and revision>1:
                        change={'type':'patches','revision':revision,'baseRevision':revision-1,'patches':[{'op':'add','path':['turns',0,'items',999],'value':{'id':'done','type':'commandExecution','status':'completed'}}]}
                    if self.fault=='patches' and revision>1:
                        change={'type':'patches','revision':revision,'baseRevision':revision-1,'patches':[{'op':'replace','path':['turns',0,'items'],'value':self.state_fn(time.monotonic()-self.start)['turns'][0]['items']}]}
                    if self.fault=='bad_frame':
                        with client['lock']:client['socket'].sendall(struct.pack('<I',notify.MAX_FRAME+1))
                        client['following']=False;continue
                    self.send(client,dict(type='broadcast',method='thread-stream-state-changed',version=version,sourceClientId=self.owner(),params={'hostId':'local','conversationId':'fake-thread','change':change}))
                except OSError:client['following']=False

    def close(self):
        self.stop.set();self.server.close()
        for client in self.clients:
            try:client['socket'].shutdown(socket.SHUT_RDWR)
            except OSError:pass
        for thread in self.threads+self.workers:
            thread.join(2)
            if thread.is_alive():raise AssertionError('Fixture thread leaked')
        self.temp.cleanup()


class DeferredTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory(prefix='notify-case-',dir='/tmp');self.addCleanup(self.temp.cleanup)
        self.state_dir=Path(self.temp.name)

    def app(self,**kwargs):
        app=FakeApp(**kwargs);self.addCleanup(app.close);return app

    def dispatcher(self,app,**overrides):
        config=dict(socket=app.path,thread='fake-thread',cwd=str(ROOT),timeout=.2,quiet=.06,final_wait=1.5)
        config.update(overrides);d=notify.Dispatcher(config,self.state_dir);self.addCleanup(d.close);return d

    def complete(self,d,timeout=3):
        self.assertTrue(d.done.wait(timeout),'Dispatcher did not finish');d.close();self.assertFalse(d.thread.is_alive());return d.snapshot()

    def test_initially_idle(self):
        a=self.app();d=self.dispatcher(a);d.submit('final',final=True);self.assertTrue(self.complete(d)['accepted']);self.assertEqual(len(a.messages),1)

    def test_pending_steering_messages_are_not_tools(self):
        s=state();s['turns'][0]['items']=[{'type':'steeringUserMessage','status':'accepted'},{'type':'steered','id':'marker'}]
        a=self.app(state_fn=lambda _:s);d=self.dispatcher(a);d.submit('final',final=True);self.assertTrue(self.complete(d)['accepted'])

    def test_multiple_tools_and_quiet_reset(self):
        def current(t):return state(['a','b'] if t<.13 else ['b'] if t<.22 else [] if t<.25 else ['c'] if t<.4 else [])
        a=self.app(state_fn=current);d=self.dispatcher(a,quiet=.08);d.submit('final',final=True);self.assertTrue(self.complete(d)['accepted']);self.assertGreaterEqual(a.messages[0]['at']-a.start,.47)

    def test_wait_without_process_status(self):
        def current(t):
            s=state();s['turns'][0]['items']=[{'id':'wait','type':'sleep','completedAtMs':None if t<.2 else 1}];return s
        a=self.app(state_fn=current);d=self.dispatcher(a);d.submit('final',final=True);self.assertTrue(self.complete(d)['accepted']);self.assertGreater(a.messages[0]['at']-a.start,.24)

    def test_patch_stream_and_fragmented_frames(self):
        a=self.app(state_fn=lambda t:state(['a'] if t<.2 else []),fault='patches',fragmented=True);d=self.dispatcher(a);d.submit('final',final=True);self.assertTrue(self.complete(d)['accepted'])

    def test_final_supersedes_all_waiting_heartbeats(self):
        # Keep the tool busy until the final message is pending. Elapsed time
        # cannot establish that ordering on a loaded CI runner.
        release_tool=threading.Event()
        a=self.app(state_fn=lambda _:state([] if release_tool.is_set() else ['a']));d=self.dispatcher(a)
        for i in range(5):d.submit('alive-'+str(i))
        d.submit('final',final=True);release_tool.set()
        self.assertTrue(self.complete(d)['accepted']);self.assertEqual([x['text'] for x in a.messages],['final'])
        events=[json.loads(x) for x in (self.state_dir/'notifications.jsonl').read_text().splitlines()];self.assertEqual(sum(e['event']=='superseded' for e in events),5)

    def test_inflight_heartbeat_does_not_overwrite_pending_final(self):
        a=self.app(delayed_reply=.2);d=self.dispatcher(a,timeout=.5);d.submit('alive');self.assertTrue(a.sent.wait(1));d.submit('final',final=True)
        time.sleep(.04);self.assertEqual(d.snapshot()['kind'],'final');self.assertIsNone(d.snapshot()['accepted']);self.assertTrue(self.complete(d)['accepted']);self.assertEqual([x['text'] for x in a.messages],['alive','final'])

    def test_disconnect_before_dispatch_recovers(self):
        a=self.app(fault='disconnect_once');d=self.dispatcher(a);d.submit('final',final=True);self.assertTrue(self.complete(d)['accepted']);self.assertGreaterEqual(a.subscriptions,2);self.assertEqual(len(a.messages),1)

    def test_revision_gap_requires_new_snapshot(self):
        a=self.app(fault='gap_once');d=self.dispatcher(a);d.submit('final',final=True);self.assertTrue(self.complete(d)['accepted']);self.assertGreaterEqual(a.subscriptions,2)

    def test_owner_change_recovers(self):
        a=self.app(state_fn=lambda t:state(['a'] if t<.3 else []),owner_switch=.18);d=self.dispatcher(a);d.submit('final',final=True);self.assertTrue(self.complete(d)['accepted']);self.assertEqual(a.messages[0]['target'],'owner-2');self.assertGreaterEqual(a.subscriptions,2)

    def test_silent_owner_change_is_rediscovered_while_busy(self):
        a=self.app(state_fn=lambda t:state(['a'] if t<.3 else []),owner_switch=.18,fault='silent_owner_change');d=self.dispatcher(a,final_wait=2.5);d.submit('final',final=True);self.assertTrue(self.complete(d)['accepted']);self.assertEqual(a.messages[0]['target'],'owner-2');self.assertGreaterEqual(a.subscriptions,2)

    def test_regressing_snapshot_is_not_trusted(self):
        a=self.app(fault='regress_once');d=self.dispatcher(a);d.submit('final',final=True);self.assertTrue(self.complete(d)['accepted']);self.assertGreaterEqual(a.subscriptions,2)

    def test_brief_tool_between_idle_states_restarts_quiet_interval(self):
        a=self.app(fault='burst');d=self.dispatcher(a,quiet=.14);d.submit('final',final=True);self.assertTrue(self.complete(d)['accepted'])
        burst=min(c['burst_sent'] for c in a.clients if 'burst_sent'in c);self.assertGreaterEqual(a.messages[0]['at']-burst,.13)

    def test_final_deadline_expires_during_inflight_alive(self):
        a=self.app(delayed_reply=.25);d=self.dispatcher(a,timeout=.5,final_wait=.08);d.submit('alive');self.assertTrue(a.sent.wait(1));d.submit('final',final=True);v=self.complete(d);self.assertFalse(v['accepted']);self.assertEqual([x['text'] for x in a.messages],['alive'])

    def test_logging_failure_is_visible_and_does_not_strand_worker(self):
        (self.state_dir/'notifications.jsonl').mkdir();a=self.app();d=self.dispatcher(a);d.submit('final',final=True);v=self.complete(d);self.assertTrue(v['accepted']);self.assertIn('logging_error',v)

    def test_deadline_budget_includes_unanswered_owner_query(self):
        a=self.app(fault='no_owner_reply');d=self.dispatcher(a,timeout=2,final_wait=.2);start=time.monotonic();d.submit('final',final=True);self.assertFalse(self.complete(d)['accepted']);self.assertLess(time.monotonic()-start,.5);self.assertEqual(a.messages,[])

    def test_unknown_after_dispatch_never_retries(self):
        for fault in ['drop_after_send','no_send_reply','bad_ack']:
            with self.subTest(fault=fault):
                a=self.app(fault=fault);d=self.dispatcher(a);d.submit('final',final=True);v=self.complete(d);self.assertEqual(v['state'],'unknown');self.assertIsNone(v['accepted']);self.assertEqual(len(a.messages),1)

    def test_rpc_error_does_not_start_or_repeat_without_dispatch_proof(self):
        a=self.app(fault='rejected');d=self.dispatcher(a);d.submit('final',final=True);v=self.complete(d);self.assertIsNone(v['accepted']);self.assertEqual(v['state'],'unknown');self.assertEqual([x['method'] for x in a.messages],['thread-follower-steer-turn'])

    def test_only_explicit_inactive_rejection_allows_start(self):
        a=self.app(fault='inactive');d=self.dispatcher(a);d.submit('final',final=True);self.assertTrue(self.complete(d)['accepted']);self.assertEqual([x['method'] for x in a.messages],['thread-follower-steer-turn','thread-follower-start-turn'])

    def test_unknown_observation_and_deadlines_fail_closed(self):
        for fault in ['version','no_snapshot','bad_frame','owner_rejected']:
            with self.subTest(fault=fault):
                a=self.app(fault=fault);d=self.dispatcher(a,final_wait=.35);started=time.monotonic();d.submit('final',final=True);v=self.complete(d);self.assertEqual(v['state'],'not_delivered');self.assertEqual(a.messages,[]);self.assertLess(time.monotonic()-started,.8)

    def test_busy_timeout(self):
        a=self.app(state_fn=lambda _:state(['always']));d=self.dispatcher(a,final_wait=.25);d.submit('final',final=True);self.assertFalse(self.complete(d)['accepted']);self.assertEqual(a.messages,[])

    def test_unknown_tool_schema_is_not_idle(self):
        s=state();s['turns'][0]['items']=[{'id':'unknown','type':'new-tool'}]
        a=self.app(state_fn=lambda _:s);d=self.dispatcher(a,final_wait=.25);d.submit('final',final=True);self.assertFalse(self.complete(d)['accepted']);self.assertEqual(a.messages,[])

    def test_pending_approval_is_busy(self):
        s=state();s['requests']=[{'id':'approval','completedAtMs':None}]
        a=self.app(state_fn=lambda _:s);d=self.dispatcher(a,final_wait=.25);d.submit('final',final=True);self.assertFalse(self.complete(d)['accepted']);self.assertEqual(a.messages,[])

    def test_unsubscribe_and_close(self):
        a=self.app();d=self.dispatcher(a);d.submit('final',final=True);self.complete(d)
        end=time.monotonic()+.3
        while a.unsubscriptions==0 and time.monotonic()<end:time.sleep(.01)
        self.assertGreater(a.unsubscriptions,0)

    def test_log_compatibility_and_terminal_evidence(self):
        a=self.app();d=self.dispatcher(a,name='test');d.submit('[Screen test] Build erfolgreich',final=True);self.assertTrue(self.complete(d)['accepted'])
        records=[json.loads(line) for line in (self.state_dir/'notifications.jsonl').read_text().splitlines()];last=records[-1]
        self.assertEqual(last['message'],'Build erfolgreich');self.assertIsInstance(last['at'],str);self.assertIn('finished_at',last);self.assertEqual(last['accepted'],True);self.assertEqual(last['text'],'[Screen test] Build erfolgreich')

    def test_unsupported_response_kind_never_allows_start(self):
        for result_type in [None, 'future-unknown-result', True, 0, [], {}]:
            with self.subTest(result_type=result_type):
                def transform(method,response,result_type=result_type):
                    if method=='thread-follower-steer-turn':return {**response,'resultType':result_type}
                    return response
                a=self.app(fault='inactive',response_filter=transform);d=self.dispatcher(a);d.submit('final',final=True);v=self.complete(d)
                self.assertEqual(v['state'],'unknown');self.assertIsNone(v['accepted']);self.assertEqual([m['method'] for m in a.messages],['thread-follower-steer-turn'])

    def test_malformed_error_payload_never_allows_start(self):
        for error in [None, '', '   ', True, 7, {}, []]:
            with self.subTest(error=error):
                def transform(method,response,error=error):
                    if method=='thread-follower-steer-turn':return {**response,'error':error}
                    return response
                a=self.app(fault='inactive',response_filter=transform);d=self.dispatcher(a);d.submit('final',final=True);v=self.complete(d)
                self.assertEqual(v['state'],'unknown');self.assertIsNone(v['accepted']);self.assertEqual(len(a.messages),1)

    def test_conflicting_success_and_error_are_unknown(self):
        def transform(method,response):
            if method=='thread-follower-steer-turn':return {**response,'error':'Cannot steer conversation fake-thread because its active turn already ended'}
            return response
        a=self.app(response_filter=transform);d=self.dispatcher(a);d.submit('final',final=True);v=self.complete(d)
        self.assertEqual(v['state'],'unknown');self.assertIsNone(v['accepted']);self.assertEqual(len(a.messages),1)

    def test_conflicting_error_and_result_do_not_allow_start(self):
        def transform(method,response):
            if method=='thread-follower-steer-turn':return {**response,'result':{'result':{'turnId':'possibly-dispatched'}}}
            return response
        a=self.app(fault='inactive',response_filter=transform);d=self.dispatcher(a);d.submit('final',final=True);v=self.complete(d)
        self.assertEqual(v['state'],'unknown');self.assertIsNone(v['accepted']);self.assertEqual([m['method'] for m in a.messages],['thread-follower-steer-turn'])

    def test_framework_errors_after_possible_dispatch_remain_unknown(self):
        for error in ['thread-follower-steer-turn-timeout','thread-follower-response-method-mismatch','client-coordination-service-unavailable','permission denied']:
            with self.subTest(error=error):
                def transform(method,response,error=error):
                    if method=='thread-follower-steer-turn':return {**response,'resultType':'error','error':error}
                    return response
                a=self.app(response_filter=transform);d=self.dispatcher(a);d.submit('final',final=True);v=self.complete(d)
                self.assertEqual(v['state'],'unknown');self.assertIsNone(v['accepted']);self.assertEqual(len(a.messages),1)

    def test_start_rpc_errors_do_not_trigger_another_attempt(self):
        for error in ['thread-follower-start-turn-timeout','Cannot steer conversation fake-thread because its active turn already ended']:
            with self.subTest(error=error):
                def transform(method,response,error=error):
                    if method=='thread-follower-start-turn':return {**response,'resultType':'error','error':error}
                    return response
                a=self.app(fault='inactive',response_filter=transform);d=self.dispatcher(a);d.submit('final',final=True);v=self.complete(d)
                self.assertEqual(v['state'],'unknown');self.assertIsNone(v['accepted']);self.assertEqual([m['method'] for m in a.messages],['thread-follower-steer-turn','thread-follower-start-turn'])

    def test_invalid_owner_reply_prevents_any_dispatch(self):
        def transform(method,response):
            if method=='thread-owner-discovery':return {**response,'resultType':'future-result'}
            return response
        a=self.app(response_filter=transform);d=self.dispatcher(a,final_wait=.25);d.submit('final',final=True);v=self.complete(d)
        self.assertEqual(v['state'],'not_delivered');self.assertFalse(v['accepted']);self.assertEqual(a.messages,[])

    def test_invalid_completion_timestamp_never_means_done(self):
        for value in [False, True, '42', {}, [], -1, float('nan'), float('inf'), -float('inf')]:
            for status in [None, 'completed', 'inProgress']:
                with self.subTest(value=value,status=status):
                    s=state();item={'id':'tool','type':'sleep','completedAtMs':value}
                    if status is not None:item['status']=status
                    s['turns'][0]['items']=[item]
                    with self.assertRaises(notify.StateUnavailable):notify.tool_state(s)

    def test_unknown_status_cannot_be_overridden_by_timestamp(self):
        for status in ['new-running-state', False, 42, {}, []]:
            for completion in [None, 0, 42]:
                with self.subTest(status=status,completion=completion):
                    s=state();s['turns'][0]['items']=[{'id':'tool','type':'commandExecution','status':status,'completedAtMs':completion}]
                    with self.assertRaises(notify.StateUnavailable):notify.tool_state(s)

    def test_valid_completion_timestamps_and_known_statuses(self):
        for completion in [0, 1, 1.5]:
            s=state();s['turns'][0]['items']=[{'id':'tool','type':'sleep','completedAtMs':completion}];self.assertEqual(notify.tool_state(s),[])
        s=state();s['turns'][0]['items']=[{'id':'tool','type':'sleep','completedAtMs':None}];self.assertTrue(notify.tool_state(s))
        for status in ['completed','failed','declined','interrupted','cancelled']:
            s=state();s['turns'][0]['items']=[{'id':'tool','type':'commandExecution','status':status}];self.assertEqual(notify.tool_state(s),[])

    def test_invalid_request_completion_is_not_silently_idle(self):
        s=state();s['requests']=[{'completedAtMs':False}]
        with self.assertRaises(notify.StateUnavailable):notify.tool_state(s)

    def test_invalid_tool_lifecycle_is_withheld_on_real_fixture_stream(self):
        for item in [{'id':'wait','type':'sleep','completedAtMs':False},{'id':'tool','type':'commandExecution','status':'new-running-state','completedAtMs':42}]:
            with self.subTest(item=item):
                s=state();s['turns'][0]['items']=[item];a=self.app(state_fn=lambda _,s=s:s);d=self.dispatcher(a,final_wait=.25);d.submit('final',final=True)
                self.assertFalse(self.complete(d)['accepted']);self.assertEqual(a.messages,[])

    def test_invalid_tool_state_requires_fresh_valid_snapshot(self):
        def current(t):
            s=state()
            if t<.18:s['turns'][0]['items']=[{'id':'tool','type':'sleep','completedAtMs':False}]
            return s
        a=self.app(state_fn=current);d=self.dispatcher(a);d.submit('final',final=True);self.assertTrue(self.complete(d)['accepted']);self.assertGreater(a.messages[0]['at']-a.start,.18);self.assertGreaterEqual(a.subscriptions,2)

    def test_boolean_patch_base_revision_is_not_an_integer_revision(self):
        a=self.app(fault='boolean_base_once');d=self.dispatcher(a);d.submit('final',final=True);self.assertTrue(self.complete(d)['accepted']);self.assertGreaterEqual(a.subscriptions,2)

    def test_nonempty_invalid_ack_is_unknown_without_replay(self):
        for fault,expected_calls in [('nonempty_bad_ack',1),('empty_turn_id',1),('invalid_start_ack',2)]:
            with self.subTest(fault=fault):
                a=self.app(fault=fault);d=self.dispatcher(a);d.submit('final',final=True);v=self.complete(d)
                self.assertEqual(v['state'],'unknown');self.assertIsNone(v['accepted']);self.assertEqual(len(a.messages),expected_calls)

    def test_active_unloaded_runtime_is_never_idle(self):
        unloaded={'turns':[],'requests':[],'resumeState':'needs_resume','threadRuntimeStatus':{'type':'active'}}
        a=self.app(state_fn=lambda _:unloaded);d=self.dispatcher(a,final_wait=.25);d.submit('final',final=True)
        self.assertFalse(self.complete(d)['accepted']);self.assertEqual(a.messages,[])

    def test_active_runtime_with_only_old_turns_is_unknown(self):
        s=state();s['turns'][0]['status']='completed';s['threadRuntimeStatus']={'type':'active'}
        with self.assertRaises(notify.StateUnavailable):notify.tool_state(s)

    def test_unloaded_state_recovers_only_after_fresh_loaded_snapshot(self):
        def current(t):
            if t<.18:return {'turns':[],'resumeState':'needs_resume','threadRuntimeStatus':{'type':'active'}}
            s=state();s['threadRuntimeStatus']={'type':'active'};s['resumeState']='resumed';return s
        a=self.app(state_fn=current);d=self.dispatcher(a);d.submit('final',final=True);self.assertTrue(self.complete(d)['accepted']);self.assertGreater(a.messages[0]['at']-a.start,.18)

    def test_unconfirmed_turn_submission_is_busy(self):
        s=state();s['unconfirmedTurnSubmissions']=[{'terminal':False}];self.assertTrue(notify.tool_state(s))

    def test_fifo_log_is_rejected_without_blocking_supervisor(self):
        os.mkfifo(self.state_dir/'notifications.jsonl',0o600);a=self.app();d=self.dispatcher(a)
        start=time.monotonic();d.submit('final',final=True);d.snapshot();self.assertLess(time.monotonic()-start,.1)
        v=self.complete(d);self.assertTrue(v['accepted']);self.assertIn('logging_error',v)

    def test_symlink_log_does_not_modify_target(self):
        target=self.state_dir/'target';target.write_text('unchanged');(self.state_dir/'notifications.jsonl').symlink_to(target)
        a=self.app();d=self.dispatcher(a);d.submit('final',final=True);v=self.complete(d);self.assertTrue(v['accepted']);self.assertIn('logging_error',v);self.assertEqual(target.read_text(),'unchanged')

    def test_slow_regular_log_write_does_not_hold_supervisor_lock(self):
        entered=threading.Event();release=threading.Event();real_write=os.write
        def delayed_write(fd,data):
            entered.set();release.wait(1);return real_write(fd,data)
        a=self.app()
        with mock.patch.object(notify.os,'write',side_effect=delayed_write):
            d=self.dispatcher(a);d.submit('alive');self.assertTrue(entered.wait(1))
            try:
                start=time.monotonic();d.snapshot();d.submit('final',final=True);self.assertLess(time.monotonic()-start,.1)
            finally:release.set()
            self.assertTrue(self.complete(d)['accepted'])

    def test_invalid_array_and_object_patch_paths_fail_without_changes(self):
        for path,op in [(['turns',0,'items',999],'add'),(['turns',0,'items',-1],'add'),(['turns',0,'items',True],'add'),(['turns',0,'items','0'],'add'),(['turns',0,'items',.5],'add'),(['turns',9,'items',0],'add'),(['missing'],'replace'),(['turns',0,'items',0],'replace'),(['turns',0,'items',0],'remove')]:
            with self.subTest(path=path,operation=op):
                s=state();before=copy.deepcopy(s)
                with self.assertRaises(notify.StateUnavailable):notify.patch_state(s,{'op':op,'path':path,'value':{}})
                self.assertEqual(s,before)
        s=state();notify.patch_state(s,{'op':'add','path':['turns',0,'items',0],'value':{'type':'commandExecution','status':'completed'}});self.assertEqual(len(s['turns'][0]['items']),1)

    def test_invalid_patch_forces_resubscription_before_send(self):
        a=self.app(fault='invalid_patch_once');d=self.dispatcher(a);d.submit('final',final=True);self.assertTrue(self.complete(d)['accepted']);self.assertGreaterEqual(a.subscriptions,2)

    def test_worker_fatal_with_final_pending_keeps_final_failure(self):
        entered=threading.Event();release=threading.Event()
        def fatal(*args,**kwargs):
            entered.set();release.wait(1);raise SystemExit('Injected fatal send failure')
        a=self.app()
        with mock.patch.object(notify,'deliver',side_effect=fatal):
            d=self.dispatcher(a);d.submit('alive');self.assertTrue(entered.wait(1));d.submit('final',final=True);release.set()
            v=self.complete(d);self.assertEqual(v['kind'],'final');self.assertFalse(v['accepted']);self.assertIn('worker_error',v)

    def test_final_submitted_during_worker_failure_is_not_overwritten(self):
        failing=threading.Event();release=threading.Event();submission=[]
        class FatalDuringObservation(BaseException):
            def __str__(self):
                failing.set();release.wait(1);return 'Injected concurrent worker failure'
        def fail(*args):raise FatalDuringObservation()
        a=self.app()
        with mock.patch.object(notify.Follower,'poll',side_effect=fail):
            d=self.dispatcher(a);d.submit('alive');self.assertTrue(failing.wait(1))
            def finalize():
                try:submission.append(d.submit('final',final=True))
                except Exception as error:submission.append(error)
            caller=threading.Thread(target=finalize);caller.start();release.set();caller.join(1)
            self.assertFalse(caller.is_alive());self.assertEqual(submission,[False]);v=self.complete(d);self.assertEqual(v['kind'],'final');self.assertFalse(v['accepted']);self.assertEqual(a.messages,[])

    def test_supervisor_build_survives_fatal_notification_worker(self):
        a=self.app();config=dict(thread='fake-thread',name='fatal-worker-regression',cwd=str(ROOT),command=[sys.executable,'-c','import time;time.sleep(.6);print("BUILD_COMPLETED")'],environment={},heartbeat=.1,timeout=.2,quiet=.03,final_wait=.3,socket=a.path,started_at='2026-11-15T00:00:00+00:00')
        (self.state_dir/'launch.json').write_text(json.dumps(config))
        wrapper=r'''
import ast,sys
from pathlib import Path
root=Path(sys.argv[1]);state=Path(sys.argv[2]);sys.path.insert(0,str(root))
import screen_chat_notify as n
def fail(self,timeout):raise SystemExit('Injected notification worker failure')
n.Follower.poll=fail
body=(root/'screen-build-helper.sh').read_text().split("<<'PY'\n",1)[1].rsplit('\nPY',1)[0]
tree=ast.parse(body);assert isinstance(tree.body[-1],ast.Try);tree.body=tree.body[:-1]
sys.argv=['regression-wrapper',str(root/'screen-build-helper.sh')]
scope={};exec(compile(tree,str(root/'screen-build-helper.sh'),'exec'),scope)
raise SystemExit(scope['run'](state))
'''
        p=subprocess.run([sys.executable,'-B','-c',wrapper,str(ROOT),str(self.state_dir)],capture_output=True,text=True,timeout=3)
        s=json.loads((self.state_dir/'status.json').read_text());self.assertEqual(p.returncode,1,p.stderr);self.assertEqual(s['exit_code'],0);self.assertIsNone(s['error']);self.assertIsNone(s['signal']);self.assertGreaterEqual(s['elapsed_seconds'],.6);self.assertIn('BUILD_COMPLETED',(self.state_dir/'build.log').read_text());self.assertFalse(s['notification_accepted']);self.assertIn('worker_error',s['notification']);self.assertIn('notification_finished_at',s);self.assertEqual(a.messages,[])

    def screen_start(self,a,command,**options):
        name='private-notify-fixture-'+str(time.time_ns());sockdir=Path(self.temp.name)/'s';sockdir.mkdir(mode=0o700,exist_ok=True);env=dict(os.environ,SCREENDIR=str(sockdir))
        args=[str(ROOT/'screen-startscript.sh'),'--thread','fake-thread','--name',name,'--build-path',str(ROOT),'--state-root',self.temp.name,'--ipc-socket',a.path,'--notify-timeout','.2','--notify-quiet-seconds','.03','--notify-final-wait-seconds',str(options.get('final_wait',.7)),'--heartbeat-seconds','.1','--',*command]
        result=subprocess.run(args,env=env,capture_output=True,text=True,timeout=5);self.assertEqual(result.returncode,0,result.stderr);launch=json.loads(result.stdout);return Path(launch['state_dir']),name,env

    def screen_end(self,path,name,env):
        end=time.monotonic()+5
        while time.monotonic()<end:
            status=json.loads((path/'status.json').read_text());listing=subprocess.run(['screen','-ls'],env=env,capture_output=True,text=True,timeout=1)
            if name not in listing.stdout+listing.stderr:
                # The session can close between the first status read and -ls.
                status=json.loads((path/'status.json').read_text())
                self.assertIn('notification_finished_at',status);return status
            time.sleep(.03)
        self.fail('Owned Screen session did not finish: '+name)

    def test_build_end_persisted_while_final_waits(self):
        a=self.app(state_fn=lambda _:state(['busy']));p,name,env=self.screen_start(a,[sys.executable,'-c','import time;time.sleep(.3)'])
        end=time.monotonic()+2
        while time.monotonic()<end:
            status=json.loads((p/'status.json').read_text())
            if status['state']=='completed':break
            time.sleep(.02)
        self.assertEqual(status['exit_code'],0);self.assertIsNone(status['notification_accepted']);self.assertLess(status['elapsed_seconds'],.7)
        final=self.screen_end(p,name,env);self.assertFalse(final['notification_accepted']);self.assertEqual(final['exit_code'],0);self.assertEqual(a.messages,[])

    def test_signal_forwarding_during_deferred_alive(self):
        a=self.app(state_fn=lambda _:state(['busy']));p,name,env=self.screen_start(a,[sys.executable,'-c','import time;time.sleep(8)'],final_wait=.4)
        status=json.loads((p/'status.json').read_text());time.sleep(.15);os.kill(status['helper_pid'],signal.SIGTERM)
        final=self.screen_end(p,name,env);self.assertEqual(final['signal'],signal.SIGTERM);self.assertEqual(final['exit_code'],-signal.SIGTERM);self.assertFalse(final['notification_accepted']);self.assertEqual(a.messages,[])
        with self.assertRaises(ProcessLookupError):os.kill(status['pid'],0)

    def test_failed_build_and_successful_notification_separate(self):
        a=self.app();p,name,env=self.screen_start(a,[sys.executable,'-c','raise SystemExit(7)'])
        final=self.screen_end(p,name,env);self.assertEqual(final['exit_code'],7);self.assertTrue(final['notification_accepted']);self.assertEqual(final['message'],'Build fehlgeschlagen')


if __name__=='__main__':
    unittest.main(verbosity=2)
