@echo off

psftp ^
    -load "iBook G3" ^
    -pw "%*" ^
    -batch ^
    -b %~dp0\ibook_psftp_script.txt
