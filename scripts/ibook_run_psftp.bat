@echo off

psftp ^
    -load "iBook G3" ^
    -pw "%*" ^
    -batch ^
    -b ibook_psftp_script.txt
