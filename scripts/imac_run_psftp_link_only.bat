@echo off

psftp ^
    -load "iMac Core Duo" ^
    -pw "%*" ^
    -batch ^
    -b %~dp0\imac_link_only_psftp_script.txt
