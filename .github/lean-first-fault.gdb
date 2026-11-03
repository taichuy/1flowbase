set pagination off
set confirm off
set print frame-arguments none
set print elements 0
set debuginfod enabled off
set disable-randomization off
handle SIGSEGV stop print nopass
handle SIGBUS stop print nopass
handle SIGABRT stop print nopass
run
python
import gdb, os
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
report('PC_SYMBOL', 'info symbol $pc', 1800)
report('PC_INSTRUCTIONS', 'x/16i $pc', 4500)
frame = gdb.newest_frame()
previous_sp = None
for depth in range(96):
    if frame is None:
        break
    try:
        sp = int(frame.read_register('sp'))
        pc = int(frame.pc())
        delta = None if previous_sp is None else sp - previous_sp
        name = (frame.name() or '<unknown>')[:600]
        sal = frame.find_sal()
        location = (sal.symtab.filename + ':' + str(sal.line)) if sal.symtab else '<no-line-table>'
        print('FRAME', depth, 'sp=' + hex(sp), 'pc=' + hex(pc), 'older_sp_delta=' + str(delta), 'kind=' + str(frame.type()), name, location[:250])
        previous_sp = sp
        frame = frame.older()
    except Exception as error:
        print('UNWIND_STOP', depth, str(error)[:250])
        break
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
print('NOTE older_sp_delta is an unwind diagnostic, not an exact compiler frame size; inline/signal frames and missing unwind info require separate interpretation.')
print('FIRST_FAULT_CAPTURE_END')
end
kill
quit
