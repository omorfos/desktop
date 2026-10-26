# 61. A program's buffer written through its file

- Status: accepted
- Date: 2026-10-26
- Supersedes: [40. One unsafe block](0040-one-unsafe-block.md)

## Context

A picture of the screen is written into a buffer the program that asked
for it shares with the compositor. Smithay hands that memory over only as
a pointer, and writing through a pointer needs `unsafe`, so `omorfos-shm`
held one block of it (ADR 40), until Smithay offered a safe way.

It has not, and there is another: the memory is a file the program made,
mapped shared, so writing into the file at the buffer's place is writing
into the memory, and needs nothing unsafe. ADR 40 turned that down because
Smithay keeps the file to itself and a buffer cannot be matched to its
file from outside. It can, in a roundabout way:

- Smithay says where a buffer's pool is mapped. That is only a number,
  and taking it is safe; the memory is never read or written through it.
- `/proc/self/maps` says which file, by device and inode, is mapped
  there, shared and writable, from its start.
- Smithay keeps the program's descriptor of the file open, to map it again
  when the pool is made larger. The descriptor in `/proc/self/fd` that is
  of that device and inode is opened again, for writing.

Ways that were looked at and do not work:

- **Keeping a copy of the descriptor as the pool is made**, by handling
  `wl_shm` before Smithay does. The pool made cannot be told from the
  request (`New<WlShmPool>` gives nothing away), and what Smithay keeps
  with a pool or a buffer is its own, so the copy could never be matched
  to a buffer.
- **`/proc/self/map_files`**, which would give the file mapped at a place
  straight away: opening it needs `CAP_SYS_ADMIN`.

## Decision

`omorfos-shm` writes through the file and forbids `unsafe`, as every
crate but `omorfos-pipewire` (ADR 41) does. It stays a crate of its own:
it is the one place that reads `/proc` to reach a program's memory, and
what it reads is a function of text alone, tested as one.

- **What it checks has not changed.** The buffer is of the format and size
  expected, its last row ends inside the memory, and the part written lies
  inside the picture, in arithmetic that cannot wrap round.
- **The file must be as long as the picture.** A program can make its file
  shorter than its memory. Writing would make it longer again, so a buffer
  whose file does not reach its last row is not written to, and its
  picture fails, as one that is not as asked does.
- **A file is looked at before it is opened, and after.** Opening some
  files does something, so a descriptor is only opened once it is of the
  file wanted; and a number can be closed and given to another file in
  between, by another thread, so what was opened is looked at again.
- **A buffer's file is found once.** A buffer is always in one pool and a
  pool in one file, even when it is made larger, so the file found is kept
  with the buffer for as long as the buffer lasts, and the lookup, which
  reads two directories of `/proc`, is not done for every picture.
- **It is kept by the buffer, never by where the memory is.** Once a pool
  is let go of, another, perhaps another program's, can be mapped at the
  same place. A file kept by place would then be given a picture of the
  screen meant for the other: the test for this brings it about.
- **A file is closed with its buffer**, the next time a picture is
  written, so that what a program has let go of is not kept open for it.

## Consequences

- `unsafe` is forbidden everywhere but in `omorfos-pipewire`, and
  `AGENTS.md` says so. `omorfos-shm` takes the workspace's lints again.
- Writing costs what the copy did: about a third of a millisecond for a
  picture of 640 by 480. The first picture in a buffer costs as much
  again, for the lookup, and more in a process with many files open and
  mapped; later ones do not.
- It leans on two things that are not promised: that Smithay keeps the
  descriptor open, and that `/proc` is mounted. Without either, pictures
  fail; nothing is written anywhere else. `docs/UPSTREAM.md` says how to
  check the first after an update of Smithay.
- The file is opened again with the compositor's rights. Smithay maps it
  shared and writable, which only a descriptor open for writing allows, so
  the program could write it already; where opening is refused, its
  pictures fail. A file opened again does not take on `O_APPEND`, which
  would put every row at the end.
- A program can make its file shorter between the length being looked at
  and the rows being written. The file is then made as long as the rows
  written, never longer than the pool the program said it had, and the
  pages may count against the compositor's memory. That harms only the
  program's own picture, as writing at the same time as the compositor
  always could.
- Where `stat` and `/proc/self/maps` do not say a file's device alike, as
  they may not for a file on a Btrfs subvolume, a pool in it is not
  found, and its pictures fail. Programs make their pools in memory (`memfd`, `/dev/shm`), where
  both agree.
- A safe way from Smithay to write into a buffer would still be simpler,
  and is still to be asked for.
