@echo off
REM Probe CPU
wmic cpu get Name,AddressWidth,NumberOfCores,NumberOfLogicalProcessors /format:list > hardware_probe.txt
REM Probe GPU
cmd /c "for /f %%i in ('wmic path Win32_VideoController get Name^,AdapterRAM /format:value ^| findstr /r ^Name=') do set GPU=%%i" >> hardware_probe.txt
wmic path Win32_VideoController get Name,AdapterRAM /format:list >> hardware_probe.txt
REM Probe RAM
wmic MemoryChip get Capacity,Manufacturer,Speed /format:list >> hardware_probe.txt
systeminfo | findstr /c:"Total Physical Memory" /c:"Available Physical Memory" >> hardware_probe.txt
