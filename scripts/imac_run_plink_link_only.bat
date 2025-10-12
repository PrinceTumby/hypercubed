@echo off

plink ^
    -load "iMac Core Duo" ^
    -pw "%*" ^
    -batch ^
    -m %~dp0\imac_link_only_plink_script.txt