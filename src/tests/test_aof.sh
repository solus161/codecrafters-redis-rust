#!/bin/bash
{
  # ACL SETUSER default >strawberry-375
  printf "*4\r\n\$3\r\nACL\r\n\$7\r\nSETUSER\r\n\$7\r\ndefault\r\n\$15\r\n>strawberry-375\r\n"

  # AUTH default wrongpassword-2916
  printf "*3\r\n\$4\r\nAUTH\r\n\$7\r\ndefault\r\n\$18\r\nwrongpassword-2916\r\n"

  # AUTH default strawberry-375
  printf "*3\r\n\$4\r\nAUTH\r\n\$7\r\ndefault\r\n\$14\r\nstrawberry-375\r\n"
} | nc localhost 6379
