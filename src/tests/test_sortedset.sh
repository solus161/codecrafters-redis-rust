#!/bin/bash
{
  printf "*4\r\n\$4\r\nZADD\r\n\$3\r\nkey\r\n\$3\r\n3.0\r\n\$4\r\nhehe\r\n"
  printf "*4\r\n\$4\r\nZADD\r\n\$3\r\nkey\r\n\$3\r\n3.1\r\n\$4\r\nhoho\r\n"
  printf "*4\r\n\$4\r\nZADD\r\n\$3\r\nkey\r\n\$3\r\n3.1\r\n\$4\r\nzzzz\r\n"

  printf "*3\r\n\$5\r\nZRANK\r\n\$3\r\nkey\r\n\$4\r\nhehe\r\n"
  printf "*3\r\n\$5\r\nZRANK\r\n\$3\r\nkey\r\n\$4\r\nzzzz\r\n"
} | nc localhost 6379
