@echo off

psftp ^
    -load "iMac Core Duo" ^
    -pw "%*" ^
    -batch ^
    -b imac_psftp_script.txt
