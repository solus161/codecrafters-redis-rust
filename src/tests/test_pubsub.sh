#!/bin/bash
{
  printf "*3\r\n\$9\r\nSUBSCRIBE\r\n\$6\r\nbanana\r\n\$5\r\ngrape\r\n"

} | nc localhost 6379
