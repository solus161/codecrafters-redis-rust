#!/bin/bash
{
  printf "*4\r\n\$4\r\nZADD\r\n\$3\r\nkey\r\n\$3\r\n3.0\r\n\$4\r\nhehe\r\n"
  printf "*4\r\n\$4\r\nZADD\r\n\$3\r\nkey\r\n\$3\r\n3.1\r\n\$4\r\nhoho\r\n"
  printf "*4\r\n\$4\r\nZADD\r\n\$3\r\nkey\r\n\$3\r\n3.1\r\n\$4\r\nzzzz\r\n"

  printf "*3\r\n\$5\r\nZRANK\r\n\$3\r\nkey\r\n\$4\r\nhehe\r\n"
  printf "*3\r\n\$5\r\nZRANK\r\n\$3\r\nkey\r\n\$4\r\nzzzz\r\n"
  
  printf "*4\r\n\$6\r\nZRANGE\r\n\$3\r\nkey\r\n\$1\r\n2\r\n\$1\r\n0\r\n"

  printf "*5\r\n\$6\r\nGEOADD\r\n\$6\r\norange\r\n\$18\r\n-165.0295233036336\r\n\$18\r\n40.666486875636664\r\n\$9\r\nblueberry\r\n"
} | nc localhost 6379
