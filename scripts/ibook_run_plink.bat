@echo off

plink ^
    -load "iBook G3" ^
    -pw "%*" ^
    -batch ^
    -m %~dp0\ibook_plink_script.txt