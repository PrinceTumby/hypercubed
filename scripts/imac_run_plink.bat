@echo off

plink ^
    -load "iMac Core Duo" ^
    -pw "%*" ^
    -batch ^
    -m imac_plink_script.txt