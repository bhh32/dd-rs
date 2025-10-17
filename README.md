# Overview
The dd-rs utility is meant to be a crossplatform replacement for the Linux coreutils dd utility. 

## Purpose
It's not that dd doesn't do what it's supposed to; it's more that it lacks modern touches, such as a progress bar. Eventually, I hope for this project to be on par with, 
or better than, the coreutils dd. Until then, the current state of the project is good for things such as burning bootable Linux ISO files to a USB.

## Roadmap
[x] Create a MVP that is good for burning Linux ISO files to USB  
[ ] Implement bs=BYTES  
[ ] Implement conv=CONVS  
    - [ ] notrunc (do not truncate the output file)  
    - [ ] noerror (continue after read errors)  
    - [ ] nocreate (do not create the output file)  
    - [ ] excl (fail if the output file already exists)  
    - [ ] ascii (from EBCDIC to ASCII)  
    - [ ] ebcdic (from ASCII to EBCDIC)  
    - [ ] ibm (from ASCII to alternate EBCDIC)  
    - [ ] block (pad newline-terminated records with spaced to cbs-size)  
    - [ ] unblock (replace trailing spaces in cbs-size records with newline)  
    - [ ] lcase (change upper case to lower case)  
    - [ ] ucase (change lower case to upper case)  
    - [ ] sparse (try to seek rather than write all-NUL output blocks)  
    - [ ] swab (swap every pair of input bytes - will change this to swap from swab)  
    - [ ] sync (pad every input blcok with NULs to ibs-size; when used with block or unblock, pad with spaces rather than NULs)  
    - [ ] fdatasync (physcially write output file data before finishing)  
    - [ ] fsync (likewise, but also writhe metadata)  
[ ] Implement count=N  
[ ] Implement iflag=FLAGS and oflags=FLAGS  
    - [ ] append (append mode - makes sense only for output; conv=notrunc suggested)  
    - [ ] direct (use direct I/O for data)  
    - [ ] directory (fail unless a directory)  
    - [ ] dsync (use synchronized I/O for data)  
    - [ ] sync (likewise, but also for metadata)  
    - [ ] fullblock (accumulate full blocks of input - iflag only)  
    - [ ] nonblock (use non-blocking I/O)  
    - [ ] noatime (do not update access time)  
    - [ ] nocache (Request to drop cache. See also oflag=sync)  
    - [ ] noctty (do not assign controlling terminal from file)  
    - [ ] nofollow (do not follow symlinks)  
