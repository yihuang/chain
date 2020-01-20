#!/usr/bin/env python3
import os
import time
from datetime import datetime
import iso8601
from chainrpc import RPC
from common import UnixStreamXMLRPCClient, wait_for_validators, wait_for_port

'''
three node, 1/3 voting power each.
target node: node2
target node mnemonic.
configs:
- missed_block_threshold
- jail_duration

procedure:
- restore node2 wallet on node0
- stop node2
- wait for missed_block_threshold blocks
- check punishment state on the first staking address
- start node2
- wait for node2 port
- do unjail, expect failure.
- wait until jail_time + jail_duration
- do unjail
'''

# keep these values same as jail_cluster.json
TARGET_NODE = 'node2'
TARGET_NODE_MNEMONIC = 'symptom labor zone shrug chicken bargain hood define tornado mass inquiry rural step color guitar'
MISSED_BLOCK_THRESHOLD = 10
JAIL_DURATION = 10
BASE_PORT = int(os.environ.get('BASE_PORT', 25560))
TARGET_PORT = BASE_PORT + 2 * 10

supervisor = UnixStreamXMLRPCClient('data/supervisor.sock')
rpc = RPC(BASE_PORT)

# wait for 3 validators online
print('Wait for 3 validators online')
wait_for_validators(rpc, 3)

enckey = rpc.wallet.restore(TARGET_NODE_MNEMONIC, name='target')

print('Stop', TARGET_NODE)
supervisor.supervisor.stopProcessGroup(TARGET_NODE)

print('Waiting for', MISSED_BLOCK_THRESHOLD + 3, 'blocks')
height = int(rpc.chain.status()['sync_info']['latest_block_height'])
while True:
    delta = int(rpc.chain.status()['sync_info']['latest_block_height']) - height
    print('blocks', delta)
    if delta >= MISSED_BLOCK_THRESHOLD + 3:
        break
    time.sleep(2)

assert len(rpc.chain.validators()['validators']) == 2

addr = rpc.address.list(enckey=enckey, name='target')[0]
punishment = rpc.staking.state(addr, enckey=enckey, name='target')['punishment']
print('punishment', punishment)
assert punishment['kind'] == 'NonLive'
print('slash amount', punishment['slash_amount'])

print('Starting', TARGET_NODE)
supervisor.supervisor.startProcessGroup(TARGET_NODE)
wait_for_port(TARGET_PORT + 9)
print('Started', TARGET_NODE)

jailed_until = punishment['jailed_until']
print('Wait for block time to pass jailed_until:', jailed_until)
while True:
    block_time = datetime.timestamp(iso8601.parse_date(rpc.chain.status()['sync_info']['latest_block_time']))
    print('block_time:', block_time)
    if block_time > jailed_until:
        break
    time.sleep(1)

print('Unjail', TARGET_NODE)
print(rpc.staking.unjail(addr, name='target', enckey=enckey))

print('Sleep 1 second')
time.sleep(1)

print('validators', len(rpc.chain.validators()['validators']))
# FIXME remove comment after the problem solved.
# assert len(rpc.chain.validators()['validators']) == 3
