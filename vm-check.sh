#!/bin/bash

# vm_check.sh
# Author: Transfairs
# Date: 21st Jan 2025
# Description: This script checks whether the current system is running inside a virtual machine.
# It performs several tests, including checking for hypervisor flags, virtualisation tools,
# system manufacturer information, and BIOS vendor details. Each test outputs PASS or FAIL.
# The script concludes with a summary indicating whether the system is likely running in a VM or not.

# --- Script Start ---
echo -e "\n======================="
echo -e " Virtual Machine Check "
echo -e "=======================\n"

# Function to print the test result
echo_result() {
    if [ $1 -eq 0 ]; then
        echo -e "$2: \033[31mFAIL\033[0m"  # Red for FAIL
    else
        echo -e "$2: \033[32mPASS\033[0m"  # Green for PASS
    fi
}

# Check priviledges
sudo -v || exit 1

# Test: Check for common hypervisor entries in /proc/cpuinfo
grep -qE 'hypervisor|vmware|kvm' /proc/cpuinfo
TEST_1=$?
echo_result $TEST_1 "Check for hypervisor entries in /proc/cpuinfo"

# Test: Check for specific hypervisor modules in dmesg
DMESG_OUTPUT=$(sudo dmesg 2>/dev/null)
if [ -n "$DMESG_OUTPUT" ]; then
    echo "$DMESG_OUTPUT" | grep -iqE 'vmware|qemu|kvm|hyper-v'
    TEST_2=$?
else
    TEST_2=1
fi
echo_result $TEST_2 "Check for hypervisor modules in dmesg"

# Test: Check for VirtualBox or VMware tools installed
dpkg -l | grep -iqE 'virtualbox-guest|open-vm-tools'
TEST_3=$?
echo_result $TEST_3 "Check for VirtualBox or VMware tools"

# Test: Check for system manufacturer indicating a virtual machine
MANUFACTURER=$(sudo dmidecode -s system-manufacturer 2>/dev/null)
echo "$MANUFACTURER" | grep -iqE 'vmware|virtualbox|qemu|kvm|microsoft'
TEST_4=$?
echo_result $TEST_4 "Check system manufacturer from dmidecode"

# Test: Check for presence of hypervisor flags
grep -q "hypervisor" /proc/cpuinfo
TEST_5=$?
echo_result $TEST_5 "Check for hypervisor flag in CPU info"

# Test: Check for BIOS vendor suggesting virtualisation
BIOS_VENDOR=$(sudo dmidecode -s bios-vendor 2>/dev/null)
echo "$BIOS_VENDOR" | grep -iqE 'vmware|virtualbox|xen|qemu|kvm|microsoft'
TEST_6=$?
echo_result $TEST_6 "Check BIOS vendor from dmidecode"

# Summary of results
echo -e "\n-----------------------"
echo -e " Summary "
echo -e "-----------------------"
    echo -ne "System is likely "
if [ $TEST_1 -ne 0 ] && [ $TEST_2 -ne 0 ] && [ $TEST_3 -ne 0 ] && [ $TEST_4 -ne 0 ] && [ $TEST_5 -ne 0 ] && [ $TEST_6 -ne 0 ]; then
    echo -ne "\033[32mNOT\033[0m running"
else
    echo -ne "\033[31mrunning\033[0m"
fi
echo -e " in a virtual machine."
echo -e "\n=======================\n"
