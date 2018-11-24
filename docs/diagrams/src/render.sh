mscgen -Tsvg -o ../open_from_scratch.svg open_from_scratch.msc
mscgen -Tsvg -o ../simultaneous_opening.svg simultaneous_opening.msc
mscgen -Tsvg -o ../refill_or_withdraw.svg refill_or_withdraw.msc
dot state_machine.dot -Tsvg -o ../state_machine.svg