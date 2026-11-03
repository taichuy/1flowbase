set pagination off
set confirm off
set print frame-arguments none
set print elements 32
set debuginfod enabled off
set disable-randomization off
handle SIGSEGV stop print nopass
handle SIGBUS stop print nopass
handle SIGABRT stop print nopass
run
python
import gdb, os, re
def report(label, command, limit=4000):
    try:
        print(label, gdb.execute(command, to_string=True)[:limit])
    except Exception as error:
        print(label + '_UNAVAILABLE', str(error)[:200])
print('FIRST_FAULT_CAPTURE_BEGIN')
thread = gdb.selected_thread()
print('THREAD', thread.num if thread else None, (thread.name or '') if thread else '')
report('PROGRAM', 'info program', 1000)
report('REGISTERS', 'info registers pc sp fp', 1800)
report('FAULT_ADDRESS', 'p/x $_siginfo._sifields._sigfault.si_addr', 1000)
report('FAULT_CODE', 'p $_siginfo.si_code', 1000)
report('PC_SYMBOL', 'info symbol $pc', 1800)
report('PC_INSTRUCTIONS', 'x/16i $pc', 4500)
frame = gdb.newest_frame()
previous_sp = None
previous_pc = None
large_pairs = []
top_pc = int(frame.pc()) if frame else None
for depth in range(160):
    if frame is None:
        break
    try:
        sp = int(frame.read_register('sp'))
        pc = int(frame.pc())
        delta = None if previous_sp is None else sp - previous_sp
        name = (frame.name() or '<unknown>')[:600]
        sal = frame.find_sal()
        location = (sal.symtab.filename + ':' + str(sal.line)) if sal.symtab else '<no-line-table>'
        print('FRAME', depth, 'sp=' + hex(sp), 'pc=' + hex(pc), 'sp_minus_younger_sp=' + str(delta), 'kind=' + str(frame.type()), name, location[:250])
        if delta is not None and delta > 65536:
            large_pairs.append((delta, depth - 1, previous_pc))
        previous_sp = sp
        previous_pc = pc
        frame = frame.older()
    except Exception as error:
        print('UNWIND_STOP', depth, str(error)[:250])
        break
# The SP delta printed at depth N is the interval from younger N-1 to N.
# Its associated prologue is therefore that of the younger function, not N.
prologues = [(0, 0, top_pc)] if top_pc is not None else []
prologues += sorted(large_pairs, reverse=True)[:8]
for delta, younger_depth, pc in prologues:
    try:
        symbol = gdb.execute('info symbol ' + hex(pc), to_string=True)
        match = re.search(r' \+ ([0-9]+) in section ', symbol)
        offset = int(match.group(1)) if match else 0
        print('PROLOGUE', 'younger_depth=' + str(younger_depth), 'observed_interval=' + str(delta), symbol[:1200])
        print(gdb.execute('x/20i ' + hex(pc - offset), to_string=True)[:5500])
    except Exception as error:
        print('PROLOGUE_UNAVAILABLE', str(error)[:250])
# Map addresses only. Do not read inferior heap, locals, credential values or request bodies.
try:
    inferior = gdb.selected_inferior()
    sp = int(gdb.parse_and_eval('$sp'))
    pc = int(gdb.parse_and_eval('$pc'))
    rows = open('/proc/' + str(inferior.pid) + '/maps').read().splitlines()
    intervals = [(int(row.split()[0].split('-')[0],16), int(row.split()[0].split('-')[1],16), row) for row in rows]
    selected = set()
    for address in [sp, pc]:
        distance = lambda item: 0 if item[1][0] <= address < item[1][1] else min(abs(address-item[1][0]), abs(address-item[1][1]))
        index = min(enumerate(intervals), key=distance)[0]
        selected.update(range(max(0,index-1), min(len(rows),index+2)))
    for index in sorted(selected):
        print('NEAR_PC_OR_SP_MAPPING', rows[index][:600])
except Exception as error:
    print('MAPPINGS_UNAVAILABLE', str(error)[:250])
print('NOTE sp_minus_younger_sp at depth N measures the interval from N-1 to N, not the exact size of frame N; inline/signal frames and missing unwind info require separate interpretation.')
print('FIRST_FAULT_CAPTURE_END')
end
kill
quit
