@echo off

plink ^
    -load "iBook G3" ^
    -pw "%*" ^
    -batch ^
    -m ibook_plink_script.txt