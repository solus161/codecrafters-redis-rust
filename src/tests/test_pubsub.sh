#!/bin/bash
{
  printf "*3\r\n\$9\r\nSUBSCRIBE\r\n\$6\r\nbanana\r\n\$5\r\ngrape\r\n"
  printf "*1\r\n\$4\r\nPING\r\n"
  printf "*2\r\n\$4\r\nECHO\r\n\$4\r\nhehe\r\n"

} | nc localhost 6379
